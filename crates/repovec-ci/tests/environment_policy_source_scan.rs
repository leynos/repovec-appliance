//! Source scan closing the inner-attribute route around the policy.
//!
//! The other two contracts check the configuration and prove the lint
//! fires. Neither sees a source file that switches the lint off. A
//! crate-level inner attribute does exactly that:
//!
//! ```ignore
//! #![allow(clippy::disallowed_methods, reason = "...")]
//! ```
//!
//! `clippy::allow_attributes`, which the workspace denies, does not fire
//! on inner attributes, so the whole crate stops enforcing the policy
//! with every gate still green. Measured on `main` at 5596f65: with that
//! attribute in `repovec-ci`'s crate root and an unannotated
//! `std::env::var("HOME")` beside it, `make lint` exited 0, the
//! configuration contract passed all five tests, the Clippy fixture test
//! passed, and the workflow contracts passed all fifteen.
//!
//! `expect` is untouched by this scan. It is the sanctioned form for a
//! composition root precisely because it warns once the site grows a
//! seam; `allow` is silent forever.
//!
//! Mutation proof, recorded 2026-09-08; each applied alone and reverted:
//!
//! - `#![allow(clippy::disallowed_methods)]` in a crate root fails
//!   `no_source_file_allows_a_policy_lint`;
//! - `#[allow(warnings)]` on an item fails it;
//! - `#![allow(clippy::all)]` spread over several lines fails it, which
//!   is why the scan reads an attribute to its closing parenthesis
//!   rather than one line at a time.
//!
//! The lint list is tokenised rather than searched, after the first
//! draft would have reported `#[allow(clippy::allow_attributes)]` as
//! suppressing `clippy::all`, whose name it contains.

use std::collections::VecDeque;

use camino::{Utf8Path, Utf8PathBuf};
use cap_std::{ambient_authority, fs_utf8::Dir};

/// Lints whose suppression disarms the environment-access policy, and
/// the blanket suppressions that take it down with everything else.
///
/// Extend this when a lint becomes load-bearing for a repository policy.
const PROTECTED_LINTS: [&str; 3] = ["clippy::disallowed_methods", "warnings", "clippy::all"];

/// Directories holding Rust sources the policy governs.
const SOURCE_ROOTS: [&str; 1] = ["crates"];

/// Attribute openers. Only `allow` is rejected: `expect` is the
/// sanctioned form, because it warns once its site no longer needs it.
const ALLOW_OPENERS: [&str; 2] = ["#[allow(", "#![allow("];

/// Return the repository root, from this crate's manifest directory.
fn repository_root() -> Utf8PathBuf {
    Utf8PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("..").join("..")
}

/// Collect every `.rs` file under one root, depth first.
///
/// Paths are reported relative to the repository root, because the
/// absolute one is built from this crate's manifest directory and reads
/// as `crates/repovec-ci/../../crates/...`, which is noise in a failure
/// a contributor has to act on.
fn rust_sources(root: &Utf8Path, relative: &str) -> Result<Vec<(Utf8PathBuf, String)>, String> {
    let directory = Dir::open_ambient_dir(root, ambient_authority())
        .map_err(|error| format!("open {root}: {error}"))?;
    let mut pending = VecDeque::from([(directory, Utf8PathBuf::from(relative))]);
    let mut sources = Vec::new();

    while let Some((current, prefix)) = pending.pop_front() {
        let entries = current.entries().map_err(|error| format!("read {prefix}: {error}"))?;
        for candidate in entries {
            let entry = candidate.map_err(|error| format!("entry under {prefix}: {error}"))?;
            let name = entry.file_name().map_err(|error| format!("name: {error}"))?;
            let path = prefix.join(&name);
            let file_type =
                entry.file_type().map_err(|error| format!("type of {path}: {error}"))?;
            if file_type.is_dir() {
                let child =
                    current.open_dir(&name).map_err(|error| format!("open {path}: {error}"))?;
                pending.push_back((child, path));
            } else if path.extension() == Some("rs") {
                let contents = current
                    .read_to_string(&name)
                    .map_err(|error| format!("read {path}: {error}"))?;
                sources.push((path, contents));
            }
        }
    }
    Ok(sources)
}

/// Return the attribute beginning at `line`, joined to its closing
/// parenthesis.
///
/// An attribute is recognised only where a line begins with one, so a
/// mention inside a doc comment, a string literal or a macro body is not
/// mistaken for one. Reading on to the balancing parenthesis is what
/// catches an attribute split over several lines.
fn attribute_from(lines: &[&str], start: usize) -> Option<String> {
    let first = lines.get(start)?.trim_start();
    if !ALLOW_OPENERS.iter().any(|opener| first.starts_with(opener)) {
        return None;
    }

    let mut attribute = String::new();
    let mut depth = 0_i32;
    for line in lines.iter().skip(start) {
        attribute.push_str(line);
        attribute.push(' ');
        depth += i32::try_from(line.matches('(').count()).unwrap_or(0);
        depth -= i32::try_from(line.matches(')').count()).unwrap_or(0);
        if depth <= 0 {
            break;
        }
    }
    Some(attribute)
}

/// Return the lint names an attribute suppresses.
///
/// The list is tokenised rather than searched, because a protected name
/// can be a prefix of an innocent one: `clippy::all` sits inside
/// `clippy::allow_attributes`, so a substring test would report a crate
/// that suppresses nothing of the sort. Key-value arguments such as
/// `reason = "..."` are not lint names and are dropped.
fn lint_names(attribute: &str) -> Vec<String> {
    let normalised = attribute.replace(char::is_whitespace, "");
    let Some(open) = normalised.find('(') else {
        return Vec::new();
    };
    let Some(close) = normalised.rfind(')') else {
        return Vec::new();
    };
    let Some(inner) = normalised.get(open.saturating_add(1)..close) else {
        return Vec::new();
    };
    inner
        .split(',')
        .filter(|token| !token.is_empty() && !token.contains('='))
        .map(str::to_owned)
        .collect()
}

/// Return every protected lint suppressed by an `allow` in one file.
fn suppressed_lints(contents: &str) -> Vec<(String, String)> {
    let lines: Vec<&str> = contents.lines().collect();
    let mut found = Vec::new();
    for start in 0..lines.len() {
        let Some(attribute) = attribute_from(&lines, start) else {
            continue;
        };
        for name in lint_names(&attribute) {
            if PROTECTED_LINTS.contains(&name.as_str()) {
                found.push((name, attribute.trim().to_owned()));
            }
        }
    }
    found
}

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
            for (lint, attribute) in suppressed_lints(&contents) {
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

/// Scenario: an `expect` at a sanctioned composition root.
///
/// Invariant: the scan does not reject it. Rejecting `expect` would push
/// contributors towards `allow`, which is the attribute that never
/// warns, so the scan must leave the sanctioned form alone.
#[test]
fn a_sanctioned_expect_is_not_an_offence() {
    let sanctioned = "#[expect(clippy::disallowed_methods, reason = \"composition root\")]\n\
         fn read() -> Option<String> { std::env::var(\"HOME\").ok() }\n";

    assert!(suppressed_lints(sanctioned).is_empty());
}

/// Scenario: a lint whose name merely contains a protected one.
///
/// Invariant: `clippy::allow_attributes` is not reported as suppressing
/// `clippy::all`. A substring test would reject it, and the contributor
/// would have no way to tell a real finding from a false one.
#[test]
fn a_longer_lint_name_containing_a_protected_one_is_not_an_offence() {
    let innocent = "#[allow(clippy::allow_attributes, clippy::alloc_instead_of_core)]\n";

    assert!(suppressed_lints(innocent).is_empty());
}

/// Scenario: a doc comment or string mentions the attribute.
///
/// Invariant: only a line that begins with an attribute counts, so prose
/// describing the policy is not reported as breaking it. This file and
/// its sibling contracts both quote the attribute in their own
/// documentation.
#[test]
fn a_mention_that_does_not_begin_a_line_is_not_an_attribute() {
    let prose = "//! Never write #![allow(clippy::disallowed_methods)] in a crate root.\n\
         const EXAMPLE: &str = \"#[allow(warnings)]\";\n";

    assert!(suppressed_lints(prose).is_empty());
}
