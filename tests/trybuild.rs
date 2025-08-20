#[test]
fn ui() {
    let t = trybuild::TestCases::new();
    t.compile_fail("tests/tb/fail_*.rs");
    t.pass("tests/tb/pass_*.rs");
}
