use crate::infer::{InferCtxt, TyCtxtInferExt};
use crate::traits::ObligationCause;
use crate::traits::{TraitEngine, TraitEngineExt};

use rustc_data_structures::fx::FxHashSet;
use rustc_hir as hir;
use rustc_hir::lang_items::LangItem;
use rustc_middle::ty::query::Providers;
use rustc_middle::ty::{self, Ty, TyCtxt, TypeSuperVisitable, TypeVisitable, TypeVisitor};
use rustc_span::Span;
use std::ops::ControlFlow;

/// This method traverses the structure of `ty`, trying to find an
/// instance of an ADT (i.e. struct or enum) that doesn't implement
/// the structural-match traits, or a generic type parameter
/// (which cannot be determined to be structural-match).
///
/// The "structure of a type" includes all components that would be
/// considered when doing a pattern match on a constant of that
/// type.
///
///  * This means this method descends into fields of structs/enums,
///    and also descends into the inner type `T` of `&T` and `&mut T`
///
///  * The traversal doesn't dereference unsafe pointers (`*const T`,
///    `*mut T`), and it does not visit the type arguments of an
///    instantiated generic like `PhantomData<T>`.
///
/// The reason we do this search is Rust currently require all ADTs
/// reachable from a constant's type to implement the
/// structural-match traits, which essentially say that
/// the implementation of `PartialEq::eq` behaves *equivalently* to a
/// comparison against the unfolded structure.
///
/// For more background on why Rust has this requirement, and issues
/// that arose when the requirement was not enforced completely, see
/// Rust RFC 1445, rust-lang/rust#61188, and rust-lang/rust#62307.
pub fn search_for_structural_match_violation<'tcx>(
    span: Span,
    tcx: TyCtxt<'tcx>,
    ty: Ty<'tcx>,
) -> Option<Ty<'tcx>> {
    ty.visit_with(&mut Search { tcx, span, seen: FxHashSet::default(), adt_const_param: false })
        .break_value()
}

/// This method traverses the structure of `ty`, trying to find any
/// types that are not allowed to be used in a const generic.
///
/// This is either because the type does not implement `StructuralEq`
/// and `StructuralPartialEq`, or because the type is intentionally
/// not supported in const generics (such as floats and raw pointers,
/// which are allowed in match blocks).
pub fn search_for_adt_const_param_violation<'tcx>(
    span: Span,
    tcx: TyCtxt<'tcx>,
    ty: Ty<'tcx>,
) -> Option<Ty<'tcx>> {
    ty.visit_with(&mut Search { tcx, span, seen: FxHashSet::default(), adt_const_param: true })
        .break_value()
}

/// This method returns true if and only if `adt_ty` itself has been marked as
/// eligible for structural-match: namely, if it implements both
/// `StructuralPartialEq` and `StructuralEq` (which are respectively injected by
/// `#[derive(PartialEq)]` and `#[derive(Eq)]`).
///
/// Note that this does *not* recursively check if the substructure of `adt_ty`
/// implements the traits.
fn type_marked_structural<'tcx>(
    infcx: &InferCtxt<'_, 'tcx>,
    adt_ty: Ty<'tcx>,
    cause: ObligationCause<'tcx>,
) -> bool {
    let mut fulfillment_cx = <dyn TraitEngine<'tcx>>::new(infcx.tcx);
    // require `#[derive(PartialEq)]`
    let structural_peq_def_id =
        infcx.tcx.require_lang_item(LangItem::StructuralPeq, Some(cause.span));
    fulfillment_cx.register_bound(
        infcx,
        ty::ParamEnv::empty(),
        adt_ty,
        structural_peq_def_id,
        cause.clone(),
    );
    // for now, require `#[derive(Eq)]`. (Doing so is a hack to work around
    // the type `for<'a> fn(&'a ())` failing to implement `Eq` itself.)
    let structural_teq_def_id =
        infcx.tcx.require_lang_item(LangItem::StructuralTeq, Some(cause.span));
    fulfillment_cx.register_bound(
        infcx,
        ty::ParamEnv::empty(),
        adt_ty,
        structural_teq_def_id,
        cause,
    );

    // We deliberately skip *reporting* fulfillment errors (via
    // `report_fulfillment_errors`), for two reasons:
    //
    // 1. The error messages would mention `std::marker::StructuralPartialEq`
    //    (a trait which is solely meant as an implementation detail
    //    for now), and
    //
    // 2. We are sometimes doing future-incompatibility lints for
    //    now, so we do not want unconditional errors here.
    fulfillment_cx.select_all_or_error(infcx).is_empty()
}

/// This implements the traversal over the structure of a given type to try to
/// find instances of ADTs (specifically structs or enums) that do not implement
/// the structural-match traits (`StructuralPartialEq` and `StructuralEq`).
struct Search<'tcx> {
    span: Span,

    tcx: TyCtxt<'tcx>,

    /// Tracks ADTs previously encountered during search, so that
    /// we will not recur on them again.
    seen: FxHashSet<hir::def_id::DefId>,

    // Additionally deny things that have been allowed in patterns,
    // but are not allowed in adt const params, such as floats and
    // fn ptrs.
    adt_const_param: bool,
}

impl<'tcx> Search<'tcx> {
    fn type_marked_structural(&self, adt_ty: Ty<'tcx>) -> bool {
        adt_ty.is_structural_eq_shallow(self.tcx)
    }
}

impl<'tcx> TypeVisitor<'tcx> for Search<'tcx> {
    type BreakTy = Ty<'tcx>;

    fn visit_ty(&mut self, ty: Ty<'tcx>) -> ControlFlow<Self::BreakTy> {
        debug!("Search visiting ty: {:?}", ty);

        let (adt_def, substs) = match *ty.kind() {
            ty::Adt(adt_def, substs) => (adt_def, substs),
            ty::Param(_) => {
                return ControlFlow::Break(ty);
            }
            ty::Dynamic(..) => {
                return ControlFlow::Break(ty);
            }
            ty::Foreign(_) => {
                return ControlFlow::Break(ty);
            }
            ty::Opaque(..) => {
                return ControlFlow::Break(ty);
            }
            ty::Projection(..) => {
                return ControlFlow::Break(ty);
            }
            ty::Closure(..) => {
                return ControlFlow::Break(ty);
            }
            ty::Generator(..) | ty::GeneratorWitness(..) => {
                return ControlFlow::Break(ty);
            }
            ty::FnDef(..) => {
                // Types of formals and return in `fn(_) -> _` are also irrelevant;
                // so we do not recur into them via `super_visit_with`
                return ControlFlow::CONTINUE;
            }
            ty::Array(_, n)
                if { n.try_eval_usize(self.tcx, ty::ParamEnv::reveal_all()) == Some(0) } =>
            {
                // rust-lang/rust#62336: ignore type of contents
                // for empty array.
                return ControlFlow::CONTINUE;
            }
            ty::Bool | ty::Char | ty::Int(_) | ty::Uint(_) | ty::Str | ty::Never => {
                // These primitive types are always structural match.
                //
                // `Never` is kind of special here, but as it is not inhabitable, this should be fine.
                return ControlFlow::CONTINUE;
            }

            ty::FnPtr(..) => {
                if !self.adt_const_param {
                    return ControlFlow::CONTINUE;
                } else {
                    return ControlFlow::Break(ty);
                }
            }

            ty::RawPtr(..) => {
                if !self.adt_const_param {
                    // structural-match ignores substructure of
                    // `*const _`/`*mut _`, so skip `super_visit_with`.
                    //
                    // For example, if you have:
                    // ```
                    // struct NonStructural;
                    // #[derive(PartialEq, Eq)]
                    // struct T(*const NonStructural);
                    // const C: T = T(std::ptr::null());
                    // ```
                    //
                    // Even though `NonStructural` does not implement `PartialEq`,
                    // structural equality on `T` does not recur into the raw
                    // pointer. Therefore, one can still use `C` in a pattern.
                    return ControlFlow::CONTINUE;
                } else {
                    return ControlFlow::Break(ty);
                }
            }

            ty::Float(_) => {
                if !self.adt_const_param {
                    return ControlFlow::CONTINUE;
                } else {
                    return ControlFlow::Break(ty);
                }
            }

            ty::Array(..) | ty::Slice(_) | ty::Ref(..) | ty::Tuple(..) => {
                // First check all contained types and then tell the caller to continue searching.
                return ty.super_visit_with(self);
            }
            ty::Infer(_) | ty::Placeholder(_) | ty::Bound(..) => {
                bug!("unexpected type during structural-match checking: {:?}", ty);
            }
            ty::Error(_) => {
                self.tcx.sess.delay_span_bug(self.span, "ty::Error in structural-match check");
                // We still want to check other types after encountering an error,
                // as this may still emit relevant errors.
                return ControlFlow::CONTINUE;
            }
        };

        if !self.seen.insert(adt_def.did()) {
            debug!("Search already seen adt_def: {:?}", adt_def);
            return ControlFlow::CONTINUE;
        }

        if !self.type_marked_structural(ty) {
            debug!("Search found ty: {:?}", ty);
            return ControlFlow::Break(ty);
        }

        // structural-match does not care about the
        // instantiation of the generics in an ADT (it
        // instead looks directly at its fields outside
        // this match), so we skip super_visit_with.
        //
        // (Must not recur on substs for `PhantomData<T>` cf
        // rust-lang/rust#55028 and rust-lang/rust#55837; but also
        // want to skip substs when only uses of generic are
        // behind unsafe pointers `*const T`/`*mut T`.)

        // even though we skip super_visit_with, we must recur on
        // fields of ADT.
        let tcx = self.tcx;
        adt_def.all_fields().map(|field| field.ty(tcx, substs)).try_for_each(|field_ty| {
            let ty = self.tcx.normalize_erasing_regions(ty::ParamEnv::empty(), field_ty);
            debug!("structural-match ADT: field_ty={:?}, ty={:?}", field_ty, ty);
            ty.visit_with(self)
        })
    }
}

pub fn provide(providers: &mut Providers) {
    providers.has_structural_eq_impls = |tcx, ty| {
        tcx.infer_ctxt().enter(|infcx| {
            let cause = ObligationCause::dummy();
            type_marked_structural(&infcx, ty, cause)
        })
    };
}
