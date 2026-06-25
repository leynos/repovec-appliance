//! Compile-time contract tests for GitHub OAuth domain wrappers.

#[test]
fn github_oauth_secret_wrappers_are_opaque() {
    let tests = trybuild::TestCases::new();
    tests.compile_fail("tests/ui/github_oauth_secret_wrappers_are_opaque.rs");
}
