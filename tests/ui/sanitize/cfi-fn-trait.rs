// Check trait objects run correctly

//@needs-sanitizer-cfi
//@compile-flags: --crate-type=bin -Cprefer-dynamic=off -Clto -Zsanitizer=cfi -C codegen-units=1 -C opt-level=0
//@run-pass

fn foo() {}

static FOO: &'static (dyn Fn() + Sync) = &foo;

pub fn bar() {
    FOO()
}
fn main() {
    bar()
}
