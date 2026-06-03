//! Compile-fail tests using `trybuild`.
//!
//! These tests verify that the `#[sprachspiel::tool]` macro produces
//! human-readable error messages for invalid inputs.

#[test]
fn ui() {
    let t = trybuild::TestCases::new();
    t.compile_fail("tests/ui/*.rs");
}
