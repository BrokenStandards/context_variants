#[test]
fn ui() {
    let t = trybuild::TestCases::new();

    if let Ok(pattern) = std::env::var("TRYBUILD_FILTER") {
        t.pass(&format!("tests/tb/pass_{}*.rs", pattern));
        t.compile_fail(&format!("tests/tb/fail_{}*.rs", pattern));
    } else {
        t.pass("tests/tb/pass_*.rs");
        t.compile_fail("tests/tb/fail_*.rs");
    }
}
