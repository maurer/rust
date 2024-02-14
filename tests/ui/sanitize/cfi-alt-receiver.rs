// Check alternate receivers work

// needs-sanitizer-cfi
// compile-flags: --crate-type=bin -Cprefer-dynamic=off -Clto -Zsanitizer=cfi -C codegen-units=1 -C opt-level=0
// run-pass

// FIXME: opt-level=0 is load bearing, and shouldn't be
// FIXME: we're emitting a warning for an unexpected initial local type, need to deal with that or
// verify it happens without shims

use std::sync::Arc;

trait Fooable {
    fn foo(self: Arc<Self>);
}

struct Bar;

impl Fooable for Bar {
    fn foo(self: Arc<Self>) {}
}

fn main() {
    let bar: Arc<dyn Fooable> = Arc::new(Bar);
    bar.foo();
}
