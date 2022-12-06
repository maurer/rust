# `cfi_name`

The tracking issue for this feature is: [#89653]

[#89653]: https://github.com/rust-lang/rust/issues/89653

------------------------

The `cfi_name` feature adds support for the `cfi_name` attribute which can be
attached to a type in order to use a different representation when computing
the type name for CFI purposes.

The primary purpose of this feature is to allow for referencing C types like
`long` which must be encoded as `l` whether this is a `u32` or a `u64` on the
platform in question.

It may also be used to describe the proper type encoding of vendor types which
are representable in Rust.

This feature will not do anything useful without either the `cfi` or `kcfi`
sanitizers enabled.

```rust,edition2021
#![feature(cfi_name)]

#[repr(transparent)]
#[cfi_name = "l"]
struct c_long(core::ffi::c_long);
```

would create a wrapper structure which is represented as a `c_long`, but type
encoded as `l`.
