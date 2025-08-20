//! Integration tests for the `context_variants` macro using the `trybuild`
//! crate. These tests compile a variety of small crates and assert that
//! correct code passes and invalid usages fail to compile.

use trybuild::TestCases;

#[test]
fn ui() {
    let t = TestCases::new();
    t.pass("tests/ui/pass_simple.rs");
    t.pass("tests/ui/pass_generics.rs");
    t.pass("tests/ui/pass_never.rs");
    t.pass("tests/ui/pass_serde.rs");
    t.pass("tests/ui/pass_default_attrs.rs");
    t.pass("tests/ui/pass_base_only_attrs.rs");
    t.pass("tests/ui/pass_complete_example.rs");
    t.pass("tests/ui/pass_field_base_only_attrs.rs");
    t.compile_fail("tests/ui/fail_unknown_variant.rs");
    t.compile_fail("tests/ui/fail_no_variants.rs");
    t.compile_fail("tests/ui/fail_unknown_skip.rs");
    t.compile_fail("tests/ui/fail_unknown_never.rs");
}