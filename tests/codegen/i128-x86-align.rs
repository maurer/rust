//[x64] only-x86_64
//[x32] only-x86

// compile-flags: -O -C no-prepopulate-passes

#![crate_type = "lib"]

// This one will use ScalarPair ABI
#[repr(C)]
#[derive(Copy, Clone)]
pub struct S32_128 {
    a: i32,
    b: i128,
}

// This one will use Struct ABI
#[repr(C)]
#[derive(Copy, Clone)]
pub struct S32_32_128 {
    a: i32,
    b: i32,
    c: i128,
}


#[no_mangle]
pub fn struct_32_128_ref(x: &S32_128) -> S32_128 {
    // CHECK: %x = alloca {.*}, align 16
    *x
}

#[no_mangle]
pub fn struct_32_32_128_ref(x: &S32_32_128) -> S32_32_128 {
    // CHECK: %x = alloca {.*}, align 16
    *x
}

#[no_mangle]
pub fn struct_32_128(x: S32_128) -> S32_128 {
    // CHECK: %x = alloca {.*}, align 16
    x
}

#[no_mangle]
pub fn struct_32_32_128(x: S32_32_128) -> S32_32_128 {
    // CHECK: %x = alloca {.*}, align 16
    x
}

#[no_mangle]
pub fn call_struct_32_128() -> S32_128 {
    struct_32_128(S32_128 {
        a: 3,
        b: 9
    })
}

#[no_mangle]
pub fn inc_128(x: Option<i128>) -> Option<i128> {
    match x {
        Some(x) => Some(x + 1),
        None => None,
    }
}

#[no_mangle]
pub fn call_inc_128() -> Option<i128> {
    inc_128(Some(3))
}

pub enum IntBorder {
    JustBefore(u128),
    AfterMax,
}

#[no_mangle]
pub fn inc_opt_int_border(x: Option<IntBorder>) -> Option<IntBorder> {
    match x {
        Some(IntBorder::JustBefore(x)) => Some(IntBorder::JustBefore(x + 7)),
        x => x,
    }
}

#[no_mangle]
pub fn call_oib() -> Option<IntBorder> {
    inc_opt_int_border(Some(IntBorder::JustBefore(10)))
}
