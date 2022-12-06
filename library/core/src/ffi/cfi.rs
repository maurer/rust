//! CFI Compatible Types, for use with LLVM CFI or KCFI
//!
//! CFI schemes commonly use the Itanium mangling of the C++ type to perform a coarse grained
//! aliasing check. Since the `c_*` family of types is implemented as aliases, which type values
//! started out as is erased by the time that CFI information is being generated.
//!
//! The family of types in this module wrap the `ffi::c_*` types and attach information about how
//! to mangle them in a C++ compatible way.
//!
//! These types should be able to be used in the same way as their unwrapped counterparts in most
//! scenarios: They implement `Deref`, `DerefMut`, `From`, and `Into` for their wrapped types.
//! They additionally provide standard arithmetic operations.
//!
//! There are three cases you will need to do something extra to use these types:
//! 1. When writing an `extern "C"` function which will be exposed as a function pointer to C, you
//!    must use these types rather than the ones in `ffi`.
//! 2. When calling such a function, you will need to use `c_foo::from()` or `.into()` to map from
//!    existing scalars into these types.
//! 3. For functions which cannot use a reference, you will either need to explicitly `*&` or use
//!    `From`/`Into` to exit these types.

macro_rules! arith_trait {
    ($name:ident, $trait:ident, $fn:ident) => {
        #[unstable(feature = "cfi_name", issue = "89653")]
        impl core::ops::$trait for $name {
            type Output = Self;
            fn $fn(self, rhs: $name) -> Self {
                Self(self.0.$fn(rhs.0))
            }
        }

        #[unstable(feature = "cfi_name", issue = "89653")]
        impl core::ops::$trait<super::$name> for $name {
            type Output = Self;
            fn $fn(self, rhs: super::$name) -> Self {
                Self(self.0.$fn(rhs))
            }
        }

        #[unstable(feature = "cfi_name", issue = "89653")]
        impl core::ops::$trait<$name> for super::$name {
            type Output = $name;
            fn $fn(self, rhs: $name) -> $name {
                $name(self.$fn(rhs.0))
            }
        }
    };
}

macro_rules! bit_traits {
    ($name:ident, bits) => {
        arith_trait!($name, BitAnd, bitand);
        arith_trait!($name, BitOr, bitor);
        arith_trait!($name, BitXor, bitxor);
    };
    ($name:ident, $bits:ident) => {};
}

macro_rules! cfi_name {
    ($name:ident, $repr:expr, [$($traits:ident),*], $bits:ident) => {
        #[allow(non_camel_case_types)]
        #[unstable(feature = "cfi_name", issue = "89653")]
        #[derive($($traits),*)]
        #[repr(transparent)]
        #[cfg_attr(not(bootstrap), cfi_name = $repr)]
        /// CFI wrapper type for $name
        pub struct $name(super::$name);

        #[unstable(feature = "cfi_name", issue = "89653")]
        impl core::ops::Deref for $name {
            type Target = super::$name;
            fn deref(&self) -> &Self::Target {
                &self.0
            }
        }

        #[unstable(feature = "cfi_name", issue = "89653")]
        impl core::ops::DerefMut for $name {
            fn deref_mut(&mut self) -> &mut Self::Target {
                &mut self.0
            }
        }

        #[unstable(feature = "cfi_name", issue = "89653")]
        impl core::convert::From<super::$name> for $name {
            fn from(x: super::$name) -> Self {
                Self(x)
            }
        }

        arith_trait!($name, Add, add);
        arith_trait!($name, Sub, sub);
        arith_trait!($name, Div, div);
        arith_trait!($name, Mul, mul);
        bit_traits!($name, $bits);
    };

    ($name:ident, $repr:expr, full_traits, $bits:ident) => {
        cfi_name!($name, $repr, [Clone, Copy, Debug, PartialEq, PartialOrd, Eq, Ord], $bits);
    };
    ($name:ident, $repr:expr, partial_traits, $bits:ident) => {
        cfi_name!($name, $repr, [Clone, Copy, Debug, PartialEq, PartialOrd], $bits);
    };
    ($name:ident, $repr:expr) => {
        cfi_name!($name, $repr, full_traits, bits);
    };
}

cfi_name!(c_char, "c");
cfi_name!(c_schar, "a");
cfi_name!(c_short, "s");
cfi_name!(c_int, "i");
cfi_name!(c_long, "l");
cfi_name!(c_longlong, "x");
// FIXME(maurer): c_ssize_t technically needs to have a different representation depending on
// whether the platform encodes it as a long vs a long long.
// In the interests of a prototype, I'm pretending that all systems use a long long. This should be
// replaced before landing.
cfi_name!(c_ssize_t, "l");
cfi_name!(c_uchar, "h");
cfi_name!(c_ushort, "t");
cfi_name!(c_uint, "i");
cfi_name!(c_ulong, "m");
cfi_name!(c_ulonglong, "y");
// FIXME(maurer): As with c_size_t, the encoding of this is platform dependent. Pretending
// everything uses a ulonglong here.
cfi_name!(c_size_t, "y");
cfi_name!(c_float, "f", partial_traits, nobits);
cfi_name!(c_double, "d", partial_traits, nobits);
