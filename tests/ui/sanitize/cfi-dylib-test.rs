//@needs-sanitizer-cfi
//@compile-flags: -Clinker-plugin-lto -C link-args=-fuse-ld=lld -Zsanitizer=cfi -C codegen-units=1 -C opt-level=0 --test
//@run-pass

#[test]
fn foo() {
    std::fs::File::open("boom").expect_err("baboom");
}
