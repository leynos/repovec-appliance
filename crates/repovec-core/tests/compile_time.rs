//! Compile-time contract tests for the crate's opaque capability types.
//!
//! `trybuild` derives its generated project directory from the *package* name
//! alone (`target/tests/trybuild/repovec-core`) and names each fixture binary
//! by its position within a call (`trybuild000`), so every driver in this
//! package shares one generated project and one set of binary names. A test
//! runner that starts each test in its own process — `cargo nextest` is one —
//! can therefore run two drivers at once: the second rewrites the generated
//! `Cargo.toml`, the first compiles a sibling's fixture under its own borrowed
//! name, and because `trybuild` discards diagnostics whose source path is not
//! one of its own fixtures, it records no error and falls back to its
//! `success: true` default. The run then fails with the misleading "expected
//! test case to fail to compile, but it succeeded" (observed 2026-10-16).
//! `trybuild` does take a lock around the generated project, but it is a
//! best-effort mtime heartbeat and cannot be relied on to serialise drivers
//! under load.
//!
//! Keep exactly one driver in this package: add fixtures to the call below
//! rather than a second file or a second test function. Upstream `trybuild`
//! follows the same shape in its own suite.

#[test]
fn opaque_secrets_and_observer_bounds_fail_to_compile() {
    let tests = trybuild::TestCases::new();
    tests.compile_fail("tests/ui/github_oauth_secret_wrappers_are_opaque.rs");
    tests.compile_fail("tests/ui/qdrant_api_key_is_opaque.rs");
    tests.compile_fail("tests/ui/qdrant_quadlet_observer_requires_send_sync.rs");
}
