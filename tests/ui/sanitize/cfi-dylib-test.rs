// needs-sanitizer-cfi
// compile-flags: -Clto -Zsanitizer=cfi -C codegen-units=1 -C opt-level=0
// run-pass

#[test]
fn foo() {
    std::fs::File::open("boom").expect("baboom");
}
