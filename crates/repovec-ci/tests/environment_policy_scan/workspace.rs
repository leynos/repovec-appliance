//! Contracts over the repository's own sources, and over the scan's error
//! paths.
//!
//! The judgement tests in the test root work on samples. These two ask a
//! different question: does the scan actually read this repository, and does it
//! say something useful when it cannot.

use crate::{
    scan::suppressed_lints,
    sources::{SOURCE_ROOTS, repository_root, rust_sources},
};

/// Scenario: a source file switches the policy lint off for itself.
///
/// Invariant: no Rust source allows a protected lint. An inner attribute
/// is the case that matters, because `clippy::allow_attributes` cannot
/// see one, so nothing else in the repository would notice.
#[test]
fn no_source_file_allows_a_policy_lint() {
    let root = repository_root();
    let mut offences = Vec::new();

    for source_root in SOURCE_ROOTS {
        let sources = rust_sources(&root.join(source_root), source_root)
            .expect("workspace sources should be readable");
        assert!(!sources.is_empty(), "{source_root} should contain Rust sources to scan");
        for (path, contents) in sources {
            let offending = suppressed_lints(&contents)
                .unwrap_or_else(|error| panic!("{path} should parse as Rust: {error}"));
            for (lint, attribute) in offending {
                offences.push(format!("{path} allows {lint} via {attribute}"));
            }
        }
    }

    assert!(
        offences.is_empty(),
        "no source may allow a protected lint; use an item-scoped \
         #[expect(..., reason = \"...\")] at a composition root instead:\n{}",
        offences.join("\n")
    );
}

/// Scenario: the scan is pointed at sources that no longer exist.
///
/// Invariant: the scan reads real files and would fail loudly rather
/// than pass by finding nothing, which is how a source scan usually
/// rots.
#[test]
fn the_scan_reads_the_workspace_sources() {
    let root = repository_root();
    let sources = rust_sources(&root.join("crates"), "crates").expect("crates should be readable");

    assert!(
        sources.len() > 20,
        "the scan should cover the workspace's sources, found {}",
        sources.len()
    );
    assert!(
        sources.iter().any(|(path, _)| path.as_str().ends_with("repovec-ci/src/lib.rs")),
        "the scan should reach each crate's root module"
    );
}

/// Scenario: a source root that is not there.
///
/// Invariant: the walk returns an error naming the path, rather than an
/// empty list. An empty list would read as "nothing allows a protected
/// lint", which is how a scan passes by finding nothing.
#[test]
fn a_missing_source_root_is_an_error_not_an_empty_scan() {
    let root = repository_root();

    let error = rust_sources(&root.join("no-such-root"), "no-such-root")
        .expect_err("a missing root must not read as an empty scan");

    assert!(error.contains("no-such-root"), "the error should name the path, got {error:?}");
}

/// Scenario: a file under a source root does not parse as Rust.
///
/// Invariant: the parse failure is returned and names the reason. The
/// workspace contract turns it into a panic naming the file, because a
/// source the scan cannot read is a source the scan is not covering.
#[test]
fn a_source_that_is_not_rust_is_an_error() {
    let error = suppressed_lints("this is not Rust {\n")
        .expect_err("unparsable input must not read as a clean file");

    assert!(error.starts_with("parse: "), "the error should name the parse failure, got {error:?}");
}
