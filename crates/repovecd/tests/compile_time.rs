//! Compile-time contract test for the adapter's opaque token.
//!
//! `trybuild` derives its generated project directory from the *package* name
//! alone (`target/tests/trybuild/repovecd`) and names each fixture binary by
//! its position within a call (`trybuild000`), so every driver in this
//! package shares one generated project and one set of binary names. A test
//! runner that starts each test in its own process — `cargo nextest` is one —
//! can therefore run two drivers at once: the second rewrites the generated
//! `Cargo.toml`, the first compiles a sibling's fixture under its own borrowed
//! name, and because `trybuild` discards diagnostics whose source path is not
//! one of its own fixtures, it records no error and falls back to its
//! `success: true` default. The run then fails with the misleading "expected
//! test case to fail to compile, but it succeeded".
//!
//! Keep exactly one driver in this package: add fixtures to the call below
//! rather than a second file or a second test function.

#[test]
fn installation_token_field_and_accessor_stay_private() {
    let tests = trybuild::TestCases::new();
    tests.compile_fail("tests/ui/installation_token_is_opaque.rs");
}
