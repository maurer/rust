//@ compile-flags: -Z warn-stack-size=42

#![crate_type = "lib"]

extern "C" {
    fn small_user(_: &Small);
    fn big_user(_: &Big);
}

struct Small(u32);

struct Big([u8; 4096]);

#[no_mangle]
pub fn small() {
    // CHECK: @small() unnamed_addr #0
    let small = Small(7);
    unsafe { small_user(&small) };
}

#[no_mangle]
pub fn big() {
    // CHECK: @big() unnamed_addr #0
    let big = Big([1; 4096]);
    unsafe { big_user(&big) };
}
// CHECK: attributes #0 = { {{.*}}"warn-stack-size"="42"{{.*}} }
