// compile-flags: --crate-type=rlib

#[repr(transparent)]
#[cfi_name = "h"] //~ERROR the `#[cfi_name]` attribute is an experimental feature
pub struct UnsignedChar(u8);
