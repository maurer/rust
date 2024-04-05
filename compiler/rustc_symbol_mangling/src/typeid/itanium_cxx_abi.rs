/// Type metadata identifiers (using Itanium C++ ABI mangling for encoding) for LLVM Control Flow
/// Integrity (CFI) and cross-language LLVM CFI support.
///
/// Encodes type metadata identifiers for LLVM CFI and cross-language LLVM CFI support using Itanium
/// C++ ABI mangling for encoding with vendor extended type qualifiers and types for Rust types that
/// are not used across the FFI boundary.
///
/// For more information about LLVM CFI and cross-language LLVM CFI support for the Rust compiler,
/// see design document in the tracking issue #89653.
use rustc_data_structures::base_n;
use rustc_data_structures::fx::FxHashMap;
use rustc_hir as hir;
use rustc_middle::ty::layout::IntegerExt;
use rustc_middle::ty::{
    self, Const, ExistentialPredicate, FloatTy, FnSig, IntTy, List, Region, RegionKind, TermKind,
    Ty, TyCtxt, UintTy,
};
use rustc_middle::ty::{GenericArg, GenericArgKind, GenericArgsRef};
use rustc_span::def_id::DefId;
use rustc_span::sym;
use rustc_target::abi::call::{Conv, FnAbi, PassMode};
use rustc_target::abi::Integer;
use rustc_target::spec::abi::Abi;
use std::fmt::Write as _;

use crate::typeid;

/// Type and extended type qualifiers.
#[derive(Eq, Hash, PartialEq)]
enum TyQ {
    None,
    Const,
    Mut,
}

/// Substitution dictionary key.
#[derive(Eq, Hash, PartialEq)]
enum DictKey<'tcx> {
    Ty(Ty<'tcx>, TyQ),
    Region(Region<'tcx>),
    Const(Const<'tcx>),
    Predicate(ExistentialPredicate<'tcx>),
}

/// Converts a number to a disambiguator (see
/// <https://rust-lang.github.io/rfcs/2603-rust-symbol-name-mangling-v0.html>).
fn to_disambiguator(num: u64) -> String {
    if let Some(num) = num.checked_sub(1) {
        format!("s{}_", base_n::encode(num as u128, 62))
    } else {
        "s_".to_string()
    }
}

/// Converts a number to a sequence number (see
/// <https://itanium-cxx-abi.github.io/cxx-abi/abi.html#mangle.seq-id>).
fn to_seq_id(num: usize) -> String {
    if let Some(num) = num.checked_sub(1) {
        base_n::encode(num as u128, 36).to_uppercase()
    } else {
        "".to_string()
    }
}

/// Substitutes a component if found in the substitution dictionary (see
/// <https://itanium-cxx-abi.github.io/cxx-abi/abi.html#mangling-compression>).
fn compress<'tcx>(
    dict: &mut FxHashMap<DictKey<'tcx>, usize>,
    key: DictKey<'tcx>,
    comp: &mut String,
) {
    match dict.get(&key) {
        Some(num) => {
            comp.clear();
            let _ = write!(comp, "S{}_", to_seq_id(*num));
        }
        None => {
            dict.insert(key, dict.len());
        }
    }
}

/// Encodes a const using the Itanium C++ ABI as a literal argument (see
/// <https://itanium-cxx-abi.github.io/cxx-abi/abi.html#mangling.literal>).
fn encode_const<'tcx>(
    tcx: TyCtxt<'tcx>,
    c: Const<'tcx>,
    dict: &mut FxHashMap<DictKey<'tcx>, usize>,
    options: typeid::Options,
) -> String {
    // L<element-type>[n][<element-value>]E as literal argument
    let mut s = String::from('L');

    match c.kind() {
        // Const parameters
        ty::ConstKind::Param(..) => {
            // L<element-type>E as literal argument

            // Element type
            s.push_str(&encode_ty(tcx, c.ty(), dict, options));
        }

        // Literal arguments
        ty::ConstKind::Value(..) => {
            // L<element-type>[n]<element-value>E as literal argument

            // Element type
            s.push_str(&encode_ty(tcx, c.ty(), dict, options));

            // The only allowed types of const values are bool, u8, u16, u32,
            // u64, u128, usize i8, i16, i32, i64, i128, isize, and char. The
            // bool value false is encoded as 0 and true as 1.
            match c.ty().kind() {
                ty::Int(ity) => {
                    let bits = c.eval_bits(tcx, ty::ParamEnv::reveal_all());
                    let val = Integer::from_int_ty(&tcx, *ity).size().sign_extend(bits) as i128;
                    if val < 0 {
                        s.push('n');
                    }
                    let _ = write!(s, "{val}");
                }
                ty::Uint(_) => {
                    let val = c.eval_bits(tcx, ty::ParamEnv::reveal_all());
                    let _ = write!(s, "{val}");
                }
                ty::Bool => {
                    let val = c.try_eval_bool(tcx, ty::ParamEnv::reveal_all()).unwrap();
                    let _ = write!(s, "{val}");
                }
                _ => {
                    bug!("encode_const: unexpected type `{:?}`", c.ty());
                }
            }
        }

        _ => {
            bug!("encode_const: unexpected kind `{:?}`", c.kind());
        }
    }

    // Close the "L..E" pair
    s.push('E');

    compress(dict, DictKey::Const(c), &mut s);

    s
}

/// Encodes a FnSig using the Itanium C++ ABI with vendor extended type qualifiers and types for
/// Rust types that are not used at the FFI boundary.
#[instrument(level = "trace", skip(tcx, dict))]
fn encode_fnsig<'tcx>(
    tcx: TyCtxt<'tcx>,
    fn_sig: &FnSig<'tcx>,
    dict: &mut FxHashMap<DictKey<'tcx>, usize>,
    mut options: typeid::Options,
) -> String {
    // Function types are delimited by an "F..E" pair
    let mut s = String::from("F");

    match fn_sig.abi {
        Abi::C { .. } => options.insert(typeid::Options::GENERALIZE_REPR_C),
        _ => options.remove(typeid::Options::GENERALIZE_REPR_C),
    }

    // Encode the return type
    let ty = typeid::ty::transform(tcx, options, fn_sig.output());
    s.push_str(&encode_ty(tcx, ty, dict, options));

    // Encode the parameter types
    let tys = fn_sig.inputs();
    if !tys.is_empty() {
        for ty in tys {
            let ty = typeid::ty::transform(tcx, options, *ty);
            s.push_str(&encode_ty(tcx, ty, dict, options));
        }

        if fn_sig.c_variadic {
            s.push('z');
        }
    } else {
        if fn_sig.c_variadic {
            s.push('z');
        } else {
            // Empty parameter lists, whether declared as () or conventionally as (void), are
            // encoded with a void parameter specifier "v".
            s.push('v')
        }
    }

    // Close the "F..E" pair
    s.push('E');

    s
}

/// Encodes a predicate using the Itanium C++ ABI with vendor extended type qualifiers and types for
/// Rust types that are not used at the FFI boundary.
fn encode_predicate<'tcx>(
    tcx: TyCtxt<'tcx>,
    predicate: ty::PolyExistentialPredicate<'tcx>,
    dict: &mut FxHashMap<DictKey<'tcx>, usize>,
    options: typeid::Options,
) -> String {
    // u<length><name>[I<element-type1..element-typeN>E], where <element-type> is <subst>, as vendor
    // extended type.
    let mut s = String::new();
    match predicate.as_ref().skip_binder() {
        ty::ExistentialPredicate::Trait(trait_ref) => {
            let name = encode_ty_name(tcx, trait_ref.def_id);
            let _ = write!(s, "u{}{}", name.len(), &name);
            s.push_str(&encode_args(tcx, trait_ref.args, dict, options));
        }
        ty::ExistentialPredicate::Projection(projection) => {
            let name = encode_ty_name(tcx, projection.def_id);
            let _ = write!(s, "u{}{}", name.len(), &name);
            s.push_str(&encode_args(tcx, projection.args, dict, options));
            match projection.term.unpack() {
                TermKind::Ty(ty) => s.push_str(&encode_ty(tcx, ty, dict, options)),
                TermKind::Const(c) => s.push_str(&encode_const(tcx, c, dict, options)),
            }
        }
        ty::ExistentialPredicate::AutoTrait(def_id) => {
            let name = encode_ty_name(tcx, *def_id);
            let _ = write!(s, "u{}{}", name.len(), &name);
        }
    };
    compress(dict, DictKey::Predicate(*predicate.as_ref().skip_binder()), &mut s);
    s
}

/// Encodes predicates using the Itanium C++ ABI with vendor extended type qualifiers and types for
/// Rust types that are not used at the FFI boundary.
fn encode_predicates<'tcx>(
    tcx: TyCtxt<'tcx>,
    predicates: &List<ty::PolyExistentialPredicate<'tcx>>,
    dict: &mut FxHashMap<DictKey<'tcx>, usize>,
    options: typeid::Options,
) -> String {
    // <predicate1[..predicateN]>E as part of vendor extended type
    let mut s = String::new();
    let predicates: Vec<ty::PolyExistentialPredicate<'tcx>> = predicates.iter().collect();
    for predicate in predicates {
        s.push_str(&encode_predicate(tcx, predicate, dict, options));
    }
    s
}

/// Encodes a region using the Itanium C++ ABI as a vendor extended type.
fn encode_region<'tcx>(region: Region<'tcx>, dict: &mut FxHashMap<DictKey<'tcx>, usize>) -> String {
    // u6region[I[<region-disambiguator>][<region-index>]E] as vendor extended type
    let mut s = String::new();
    match region.kind() {
        RegionKind::ReBound(debruijn, r) => {
            s.push_str("u6regionI");
            // Debruijn index, which identifies the binder, as region disambiguator
            let num = debruijn.index() as u64;
            if num > 0 {
                s.push_str(&to_disambiguator(num));
            }
            // Index within the binder
            let _ = write!(s, "{}", r.var.index() as u64);
            s.push('E');
            compress(dict, DictKey::Region(region), &mut s);
        }
        // FIXME(@lcnr): Why is `ReEarlyParam` reachable here.
        RegionKind::ReEarlyParam(..) | RegionKind::ReErased => {
            s.push_str("u6region");
            compress(dict, DictKey::Region(region), &mut s);
        }
        RegionKind::ReLateParam(..)
        | RegionKind::ReStatic
        | RegionKind::ReError(_)
        | RegionKind::ReVar(..)
        | RegionKind::RePlaceholder(..) => {
            bug!("encode_region: unexpected `{:?}`", region.kind());
        }
    }
    s
}

/// Encodes args using the Itanium C++ ABI with vendor extended type qualifiers and types for Rust
/// types that are not used at the FFI boundary.
fn encode_args<'tcx>(
    tcx: TyCtxt<'tcx>,
    args: GenericArgsRef<'tcx>,
    dict: &mut FxHashMap<DictKey<'tcx>, usize>,
    options: typeid::Options,
) -> String {
    // [I<subst1..substN>E] as part of vendor extended type
    let mut s = String::new();
    let args: Vec<GenericArg<'_>> = args.iter().collect();
    if !args.is_empty() {
        s.push('I');
        for arg in args {
            match arg.unpack() {
                GenericArgKind::Lifetime(region) => {
                    s.push_str(&encode_region(region, dict));
                }
                GenericArgKind::Type(ty) => {
                    s.push_str(&encode_ty(tcx, ty, dict, options));
                }
                GenericArgKind::Const(c) => {
                    s.push_str(&encode_const(tcx, c, dict, options));
                }
            }
        }
        s.push('E');
    }
    s
}

/// Encodes a ty:Ty name, including its crate and path disambiguators and names.
fn encode_ty_name(tcx: TyCtxt<'_>, def_id: DefId) -> String {
    // Encode <name> for use in u<length><name>[I<element-type1..element-typeN>E], where
    // <element-type> is <subst>, using v0's <path> without v0's extended form of paths:
    //
    // N<namespace-tagN>..N<namespace-tag1>
    // C<crate-disambiguator><crate-name>
    // <path-disambiguator1><path-name1>..<path-disambiguatorN><path-nameN>
    //
    // With additional tags for DefPathData::Impl and DefPathData::ForeignMod. For instance:
    //
    //     pub type Type1 = impl Send;
    //     let _: Type1 = <Struct1<i32>>::foo;
    //     fn foo1(_: Type1) { }
    //
    //     pub type Type2 = impl Send;
    //     let _: Type2 = <Trait1<i32>>::foo;
    //     fn foo2(_: Type2) { }
    //
    //     pub type Type3 = impl Send;
    //     let _: Type3 = <i32 as Trait1<i32>>::foo;
    //     fn foo3(_: Type3) { }
    //
    //     pub type Type4 = impl Send;
    //     let _: Type4 = <Struct1<i32> as Trait1<i32>>::foo;
    //     fn foo3(_: Type4) { }
    //
    // Are encoded as:
    //
    //     _ZTSFvu29NvNIC1234_5crate8{{impl}}3fooIu3i32EE
    //     _ZTSFvu27NvNtC1234_5crate6Trait13fooIu3dynIu21NtC1234_5crate6Trait1Iu3i32Eu6regionES_EE
    //     _ZTSFvu27NvNtC1234_5crate6Trait13fooIu3i32S_EE
    //     _ZTSFvu27NvNtC1234_5crate6Trait13fooIu22NtC1234_5crate7Struct1Iu3i32ES_EE
    //
    // The reason for not using v0's extended form of paths is to use a consistent and simpler
    // encoding, as the reasoning for using it isn't relevant for type metadata identifiers (i.e.,
    // keep symbol names close to how methods are represented in error messages). See
    // https://rust-lang.github.io/rfcs/2603-rust-symbol-name-mangling-v0.html#methods.
    let mut s = String::new();

    // Start and namespace tags
    let mut def_path = tcx.def_path(def_id);
    def_path.data.reverse();
    for disambiguated_data in &def_path.data {
        s.push('N');
        s.push_str(match disambiguated_data.data {
            hir::definitions::DefPathData::Impl => "I", // Not specified in v0's <namespace>
            hir::definitions::DefPathData::ForeignMod => "F", // Not specified in v0's <namespace>
            hir::definitions::DefPathData::TypeNs(..) => "t",
            hir::definitions::DefPathData::ValueNs(..) => "v",
            hir::definitions::DefPathData::Closure => "C",
            hir::definitions::DefPathData::Ctor => "c",
            hir::definitions::DefPathData::AnonConst => "k",
            hir::definitions::DefPathData::OpaqueTy => "i",
            hir::definitions::DefPathData::CrateRoot
            | hir::definitions::DefPathData::Use
            | hir::definitions::DefPathData::GlobalAsm
            | hir::definitions::DefPathData::MacroNs(..)
            | hir::definitions::DefPathData::LifetimeNs(..)
            | hir::definitions::DefPathData::AnonAdt => {
                bug!("encode_ty_name: unexpected `{:?}`", disambiguated_data.data);
            }
        });
    }

    // Crate disambiguator and name
    s.push('C');
    s.push_str(&to_disambiguator(tcx.stable_crate_id(def_path.krate).as_u64()));
    let crate_name = tcx.crate_name(def_path.krate).to_string();
    let _ = write!(s, "{}{}", crate_name.len(), &crate_name);

    // Disambiguators and names
    def_path.data.reverse();
    for disambiguated_data in &def_path.data {
        let num = disambiguated_data.disambiguator as u64;
        if num > 0 {
            s.push_str(&to_disambiguator(num));
        }

        let name = disambiguated_data.data.to_string();
        let _ = write!(s, "{}", name.len());

        // Prepend a '_' if name starts with a digit or '_'
        if let Some(first) = name.as_bytes().first() {
            if first.is_ascii_digit() || *first == b'_' {
                s.push('_');
            }
        } else {
            bug!("encode_ty_name: invalid name `{:?}`", name);
        }

        s.push_str(&name);
    }

    s
}

/// Encodes a ty:Ty using the Itanium C++ ABI with vendor extended type qualifiers and types for
/// Rust types that are not used at the FFI boundary.
fn encode_ty<'tcx>(
    tcx: TyCtxt<'tcx>,
    ty: Ty<'tcx>,
    dict: &mut FxHashMap<DictKey<'tcx>, usize>,
    options: typeid::Options,
) -> String {
    let mut typeid = String::new();

    match ty.kind() {
        // Primitive types

        // Rust's bool has the same layout as C17's _Bool, that is, its size and alignment are
        // implementation-defined. Any bool can be cast into an integer, taking on the values 1
        // (true) or 0 (false).
        //
        // (See https://rust-lang.github.io/unsafe-code-guidelines/layout/scalars.html#bool.)
        ty::Bool => {
            typeid.push('b');
        }

        ty::Int(..) | ty::Uint(..) => {
            // u<length><type-name> as vendor extended type
            let mut s = String::from(match ty.kind() {
                ty::Int(IntTy::I8) => "u2i8",
                ty::Int(IntTy::I16) => "u3i16",
                ty::Int(IntTy::I32) => "u3i32",
                ty::Int(IntTy::I64) => "u3i64",
                ty::Int(IntTy::I128) => "u4i128",
                ty::Int(IntTy::Isize) => "u5isize",
                ty::Uint(UintTy::U8) => "u2u8",
                ty::Uint(UintTy::U16) => "u3u16",
                ty::Uint(UintTy::U32) => "u3u32",
                ty::Uint(UintTy::U64) => "u3u64",
                ty::Uint(UintTy::U128) => "u4u128",
                ty::Uint(UintTy::Usize) => "u5usize",
                _ => bug!("encode_ty: unexpected `{:?}`", ty.kind()),
            });
            compress(dict, DictKey::Ty(ty, TyQ::None), &mut s);
            typeid.push_str(&s);
        }

        // Rust's f16, f32, f64, and f126 half (16-bit), single (32-bit), double (64-bit), and
        // quad (128-bit)  precision floating-point types have IEEE-754 binary16, binary32,
        // binary64, and binary128 floating-point layouts, respectively.
        //
        // (See https://rust-lang.github.io/unsafe-code-guidelines/layout/scalars.html#fixed-width-floating-point-types.)
        ty::Float(float_ty) => {
            typeid.push_str(match float_ty {
                FloatTy::F16 => "Dh",
                FloatTy::F32 => "f",
                FloatTy::F64 => "d",
                FloatTy::F128 => "g",
            });
        }

        ty::Char => {
            // u4char as vendor extended type
            let mut s = String::from("u4char");
            compress(dict, DictKey::Ty(ty, TyQ::None), &mut s);
            typeid.push_str(&s);
        }

        ty::Str => {
            // u3str as vendor extended type
            let mut s = String::from("u3str");
            compress(dict, DictKey::Ty(ty, TyQ::None), &mut s);
            typeid.push_str(&s);
        }

        ty::Never => {
            // u5never as vendor extended type
            let mut s = String::from("u5never");
            compress(dict, DictKey::Ty(ty, TyQ::None), &mut s);
            typeid.push_str(&s);
        }

        // Compound types
        // () in Rust is equivalent to void return type in C
        _ if ty.is_unit() => {
            typeid.push('v');
        }

        // Sequence types
        ty::Tuple(tys) => {
            // u5tupleI<element-type1..element-typeN>E as vendor extended type
            let mut s = String::from("u5tupleI");
            for ty in tys.iter() {
                s.push_str(&encode_ty(tcx, ty, dict, options));
            }
            s.push('E');
            compress(dict, DictKey::Ty(ty, TyQ::None), &mut s);
            typeid.push_str(&s);
        }

        ty::Array(ty0, len) => {
            // A<array-length><element-type>
            let len = len.eval_target_usize(tcx, ty::ParamEnv::reveal_all());
            let mut s = String::from("A");
            let _ = write!(s, "{}", &len);
            s.push_str(&encode_ty(tcx, *ty0, dict, options));
            compress(dict, DictKey::Ty(ty, TyQ::None), &mut s);
            typeid.push_str(&s);
        }

        ty::Slice(ty0) => {
            // u5sliceI<element-type>E as vendor extended type
            let mut s = String::from("u5sliceI");
            s.push_str(&encode_ty(tcx, *ty0, dict, options));
            s.push('E');
            compress(dict, DictKey::Ty(ty, TyQ::None), &mut s);
            typeid.push_str(&s);
        }

        // User-defined types
        ty::Adt(adt_def, args) => {
            let mut s = String::new();
            let def_id = adt_def.did();
            if let Some(cfi_encoding) = tcx.get_attr(def_id, sym::cfi_encoding) {
                // Use user-defined CFI encoding for type
                if let Some(value_str) = cfi_encoding.value_str() {
                    let value_str = value_str.to_string();
                    let str = value_str.trim();
                    if !str.is_empty() {
                        s.push_str(str);
                        // Don't compress user-defined builtin types (see
                        // https://itanium-cxx-abi.github.io/cxx-abi/abi.html#mangling-builtin and
                        // https://itanium-cxx-abi.github.io/cxx-abi/abi.html#mangling-compression).
                        let builtin_types = [
                            "v", "w", "b", "c", "a", "h", "s", "t", "i", "j", "l", "m", "x", "y",
                            "n", "o", "f", "d", "e", "g", "z", "Dh",
                        ];
                        if !builtin_types.contains(&str) {
                            compress(dict, DictKey::Ty(ty, TyQ::None), &mut s);
                        }
                    } else {
                        #[allow(
                            rustc::diagnostic_outside_of_impl,
                            rustc::untranslatable_diagnostic
                        )]
                        tcx.dcx()
                            .struct_span_err(
                                cfi_encoding.span,
                                format!("invalid `cfi_encoding` for `{:?}`", ty.kind()),
                            )
                            .emit();
                    }
                } else {
                    bug!("encode_ty: invalid `cfi_encoding` for `{:?}`", ty.kind());
                }
            } else if options.contains(typeid::Options::GENERALIZE_REPR_C) && adt_def.repr().c() {
                // For cross-language LLVM CFI support, the encoding must be compatible at the FFI
                // boundary. For instance:
                //
                //     struct type1 {};
                //     void foo(struct type1* bar) {}
                //
                // Is encoded as:
                //
                //     _ZTSFvP5type1E
                //
                // So, encode any repr(C) user-defined type for extern function types with the "C"
                // calling convention (or extern types [i.e., ty::Foreign]) as <length><name>, where
                // <name> is <unscoped-name>.
                let name = tcx.item_name(def_id).to_string();
                let _ = write!(s, "{}{}", name.len(), &name);
                compress(dict, DictKey::Ty(ty, TyQ::None), &mut s);
            } else {
                // u<length><name>[I<element-type1..element-typeN>E], where <element-type> is
                // <subst>, as vendor extended type.
                let name = encode_ty_name(tcx, def_id);
                let _ = write!(s, "u{}{}", name.len(), &name);
                s.push_str(&encode_args(tcx, args, dict, options));
                compress(dict, DictKey::Ty(ty, TyQ::None), &mut s);
            }
            typeid.push_str(&s);
        }

        ty::Foreign(def_id) => {
            // <length><name>, where <name> is <unscoped-name>
            let mut s = String::new();
            if let Some(cfi_encoding) = tcx.get_attr(*def_id, sym::cfi_encoding) {
                // Use user-defined CFI encoding for type
                if let Some(value_str) = cfi_encoding.value_str() {
                    if !value_str.to_string().trim().is_empty() {
                        s.push_str(value_str.to_string().trim());
                    } else {
                        #[allow(
                            rustc::diagnostic_outside_of_impl,
                            rustc::untranslatable_diagnostic
                        )]
                        tcx.dcx()
                            .struct_span_err(
                                cfi_encoding.span,
                                format!("invalid `cfi_encoding` for `{:?}`", ty.kind()),
                            )
                            .emit();
                    }
                } else {
                    bug!("encode_ty: invalid `cfi_encoding` for `{:?}`", ty.kind());
                }
            } else {
                let name = tcx.item_name(*def_id).to_string();
                let _ = write!(s, "{}{}", name.len(), &name);
            }
            compress(dict, DictKey::Ty(ty, TyQ::None), &mut s);
            typeid.push_str(&s);
        }

        // Function types
        ty::FnDef(def_id, args) | ty::Closure(def_id, args) => {
            // u<length><name>[I<element-type1..element-typeN>E], where <element-type> is <subst>,
            // as vendor extended type.
            let mut s = String::new();
            let name = encode_ty_name(tcx, *def_id);
            let _ = write!(s, "u{}{}", name.len(), &name);
            s.push_str(&encode_args(tcx, args, dict, options));
            compress(dict, DictKey::Ty(ty, TyQ::None), &mut s);
            typeid.push_str(&s);
        }

        ty::CoroutineClosure(def_id, args) => {
            // u<length><name>[I<element-type1..element-typeN>E], where <element-type> is <subst>,
            // as vendor extended type.
            let mut s = String::new();
            let name = encode_ty_name(tcx, *def_id);
            let _ = write!(s, "u{}{}", name.len(), &name);
            let parent_args = tcx.mk_args(args.as_coroutine_closure().parent_args());
            s.push_str(&encode_args(tcx, parent_args, dict, options));
            compress(dict, DictKey::Ty(ty, TyQ::None), &mut s);
            typeid.push_str(&s);
        }

        ty::Coroutine(def_id, args, ..) => {
            // u<length><name>[I<element-type1..element-typeN>E], where <element-type> is <subst>,
            // as vendor extended type.
            let mut s = String::new();
            let name = encode_ty_name(tcx, *def_id);
            let _ = write!(s, "u{}{}", name.len(), &name);
            // Encode parent args only
            s.push_str(&encode_args(
                tcx,
                tcx.mk_args(args.as_coroutine().parent_args()),
                dict,
                options,
            ));
            compress(dict, DictKey::Ty(ty, TyQ::None), &mut s);
            typeid.push_str(&s);
        }

        // Pointer types
        ty::Ref(region, ty0, ..) => {
            // [U3mut]u3refI<element-type>E as vendor extended type qualifier and type
            let mut s = String::new();
            s.push_str("u3refI");
            s.push_str(&encode_ty(tcx, *ty0, dict, options));
            s.push('E');
            compress(dict, DictKey::Ty(Ty::new_imm_ref(tcx, *region, *ty0), TyQ::None), &mut s);
            if ty.is_mutable_ptr() {
                s = format!("{}{}", "U3mut", &s);
                compress(dict, DictKey::Ty(ty, TyQ::Mut), &mut s);
            }
            typeid.push_str(&s);
        }

        ty::RawPtr(ptr_ty, _mutbl) => {
            // FIXME: This can definitely not be so spaghettified.
            // P[K]<element-type>
            let mut s = String::new();
            s.push_str(&encode_ty(tcx, *ptr_ty, dict, options));
            if !ty.is_mutable_ptr() {
                s = format!("{}{}", "K", &s);
                compress(dict, DictKey::Ty(*ptr_ty, TyQ::Const), &mut s);
            };
            s = format!("{}{}", "P", &s);
            compress(dict, DictKey::Ty(ty, TyQ::None), &mut s);
            typeid.push_str(&s);
        }

        ty::FnPtr(fn_sig) => {
            // PF<return-type><parameter-type1..parameter-typeN>E
            let mut s = String::from("P");
            s.push_str(&encode_fnsig(tcx, &fn_sig.skip_binder(), dict, typeid::Options::empty()));
            compress(dict, DictKey::Ty(ty, TyQ::None), &mut s);
            typeid.push_str(&s);
        }

        // Trait types
        ty::Dynamic(predicates, region, kind) => {
            // u3dynI<element-type1[..element-typeN]>E, where <element-type> is <predicate>, as
            // vendor extended type.
            let mut s = String::from(match kind {
                ty::Dyn => "u3dynI",
                ty::DynStar => "u7dynstarI",
            });
            s.push_str(&encode_predicates(tcx, predicates, dict, options));
            s.push_str(&encode_region(*region, dict));
            s.push('E');
            compress(dict, DictKey::Ty(ty, TyQ::None), &mut s);
            typeid.push_str(&s);
        }

        // Type parameters
        ty::Param(..) => {
            // u5param as vendor extended type
            let mut s = String::from("u5param");
            compress(dict, DictKey::Ty(ty, TyQ::None), &mut s);
            typeid.push_str(&s);
        }

        // Unexpected types
        ty::Alias(..)
        | ty::Bound(..)
        | ty::Error(..)
        | ty::CoroutineWitness(..)
        | ty::Infer(..)
        | ty::Placeholder(..) => {
            bug!("encode_ty: unexpected `{:?}`", ty.kind());
        }
    };

    typeid
}

/// Returns a type metadata identifier for the specified FnAbi using the Itanium C++ ABI with vendor
/// extended type qualifiers and types for Rust types that are not used at the FFI boundary.
#[instrument(level = "trace", skip(tcx))]
pub fn typeid_for_fnabi<'tcx>(
    tcx: TyCtxt<'tcx>,
    fn_abi: &FnAbi<'tcx, Ty<'tcx>>,
    mut options: typeid::Options,
) -> String {
    // A name is mangled by prefixing "_Z" to an encoding of its name, and in the case of functions
    // its type.
    let mut typeid = String::from("_Z");

    // Clang uses the Itanium C++ ABI's virtual tables and RTTI typeinfo structure name as type
    // metadata identifiers for function pointers. The typeinfo name encoding is a two-character
    // code (i.e., 'TS') prefixed to the type encoding for the function.
    typeid.push_str("TS");

    // Function types are delimited by an "F..E" pair
    typeid.push('F');

    // A dictionary of substitution candidates used for compression (see
    // https://itanium-cxx-abi.github.io/cxx-abi/abi.html#mangling-compression).
    let mut dict: FxHashMap<DictKey<'tcx>, usize> = FxHashMap::default();

    match fn_abi.conv {
        Conv::C => options.insert(typeid::Options::GENERALIZE_REPR_C),
        _ => options.remove(typeid::Options::GENERALIZE_REPR_C),
    }

    // Encode the return type
    let ty = typeid::ty::transform(tcx, options, fn_abi.ret.layout.ty);
    typeid.push_str(&encode_ty(tcx, ty, &mut dict, options));

    // Encode the parameter types

    // We erase ZSTs as we go if the argument is skipped. This is an implementation detail of how
    // MIR is currently treated by rustc, and subject to change in the future. Specifically, MIR
    // interpretation today will allow skipped arguments to simply not be passed at a call-site.
    if !fn_abi.c_variadic {
        let mut pushed_arg = false;
        for arg in fn_abi.args.iter().filter(|arg| arg.mode != PassMode::Ignore) {
            pushed_arg = true;
            let ty = typeid::ty::transform(tcx, options, arg.layout.ty);
            typeid.push_str(&encode_ty(tcx, ty, &mut dict, options));
        }
        if !pushed_arg {
            // Empty parameter lists, whether declared as () or conventionally as (void), are
            // encoded with a void parameter specifier "v".
            typeid.push('v');
        }
    } else {
        for n in 0..fn_abi.fixed_count as usize {
            if fn_abi.args[n].mode == PassMode::Ignore {
                continue;
            }
            let ty = typeid::ty::transform(tcx, options, fn_abi.args[n].layout.ty);
            typeid.push_str(&encode_ty(tcx, ty, &mut dict, options));
        }

        typeid.push('z');
    }

    // Close the "F..E" pair
    typeid.push('E');

    // Add encoding suffixes
    if options.contains(typeid::Options::NORMALIZE_INTEGERS) {
        typeid.push_str(".normalized");
    }

    if options.contains(typeid::Options::GENERALIZE_POINTERS) {
        typeid.push_str(".generalized");
    }

    typeid
}
