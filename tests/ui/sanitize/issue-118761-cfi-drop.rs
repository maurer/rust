// Validate that objects that might have custom drop can be dropped with CFI on. See #118761

// needs-sanitizer-cfi
// compile-flags: --crate-type=bin -C opt-level=0 -Cprefer-dynamic=off -Clto -Zsanitizer=cfi
// run-pass

struct Bar;
trait Fooable {}
impl Fooable for Bar {}

fn main() {
   let _: Box<dyn Fooable> = Box::new(Bar);
}
