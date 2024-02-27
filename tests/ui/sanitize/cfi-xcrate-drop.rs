//@known-bug: unknown
//@aux-build: cfi-xcrate.rs
//@needs-sanitizer-cfi
//@compile-flags: --crate-type=bin -Cprefer-dynamic=off -Clto -Zsanitizer=cfi -C codegen-units=1 -C opt-level=0
//@run-pass

extern crate cfi_xcrate;

fn main() {
    let _ = cfi_xcrate::get_i32_method();
}
