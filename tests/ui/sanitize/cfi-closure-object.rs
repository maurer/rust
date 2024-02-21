//@needs-sanitizer-cfi
//@compile-flags: --crate-type=bin -Cprefer-dynamic=off -Clto -Zsanitizer=cfi -C codegen-units=1 -C opt-level=0
//@run-pass

static FOO: &(dyn Fn() -> i32 + Sync) = &|| 3;

fn main() {
    FOO();
}
