//! Contracts over the repository's own sources, and over the scan's error
//! paths.
//!
//! The judgement tests in the test root work on samples. These two ask a
//! different question: does the scan actually read this repository, and does it
//! say something useful when it cannot.

#[cfg(unix)]
use cap_std::{ambient_authority, fs_utf8::Dir};

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

/// Scenario: a source root holds a symlink, spelled as a directory and as a
/// file.
///
/// Invariant: the walk returns an error naming the link rather than stepping
/// over it. Containment in [`crate::tokens::includes_a_scanned_path`] accepts
/// an `include!` target without resolving it, resting on every `.rs` file
/// beneath a root being read. This walk does not follow a symlink, so one
/// skipped in silence would leave a source reached through it unscanned while
/// every gate stayed green, which is the shape of every route this policy has
/// had to close.
///
/// Both targets are relative, and the file one points inside the tree. That
/// is what makes the cases discriminate rather than merely pass. `cap_std`
/// refuses to read through a link that is absolute or that leaves the
/// sandbox, so a file case spelled either way errors whether or not this walk
/// refuses anything, and proves nothing. An in-tree relative link is read
/// happily without the refusal below.
#[cfg(unix)]
#[rstest::rstest]
#[case::a_directory_symlink("linkdir", "../outside")]
#[case::a_file_symlink("linked.rs", "inner/lib.rs")]
fn a_symlink_under_a_source_root_is_an_error(#[case] link: &str, #[case] target: &str) {
    let temporary = tempfile::tempdir().expect("a temporary directory should be available");
    let base =
        camino::Utf8Path::from_path(temporary.path()).expect("the temporary path should be UTF-8");
    let tree = Dir::open_ambient_dir(base, ambient_authority())
        .expect("the temporary directory should open");

    tree.create_dir_all("outside").expect("the outside directory should be created");
    tree.write("outside/policy.rs", "// outside\n").expect("the outside source should be written");
    tree.create_dir_all("crates/inner").expect("the crate directory should be created");
    tree.write("crates/inner/lib.rs", "// inner\n").expect("the scanned source should be written");

    // The link is made with `std`, not through the `Dir` handle, because the
    // capability API will not create one whose target leaves the sandbox, and
    // an escaping target is the case under test. Writing it from outside is
    // what lets the fixture pose the question the walk has to answer.
    std::os::unix::fs::symlink(target, base.join("crates").join(link))
        .expect("the symlink should be created");

    let root = base.join("crates");
    let error =
        rust_sources(&root, "crates").expect_err("a symlink must not be stepped over in silence");

    assert!(error.contains(link), "the error should name the link, got {error:?}");
}
