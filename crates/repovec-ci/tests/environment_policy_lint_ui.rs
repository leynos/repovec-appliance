//! Behavioural coverage for the environment-access policy as Clippy applies it.
//!
//! The sibling contract test reads the configuration files. This test runs the
//! lint: it points Clippy at `tests/fixtures/environment-policy-violation`,
//! which calls all six prohibited methods, with `CLIPPY_CONF_DIR` set to the
//! repository root so the checked-in `clippy.toml` is the configuration in
//! force. A configuration that parses but never fires would pass the contract
//! test and fail this one.
//!
//! Mutation proof. Deleting the `std::env::vars_os` entry from `clippy.toml`
//! leaves that call unreported and fails the test (2026-09-06). Swapping the
//! `std::env::var` reason for the write-side remedy also fails it
//! (2026-09-07); that mutation is the reason each remedy is matched inside its
//! own diagnostic block, since three methods share each remedy string and a
//! search across the whole output passed the swap.
//!
//! The fixture is not a workspace member and has no dependencies, so the run
//! needs no network and compiles nothing but itself.

use std::{path::PathBuf, process::Command};

use tempfile::TempDir;

/// Prohibited methods paired with the remedy the diagnostic must carry.
const REPORTED_METHODS: [(&str, &str); 6] = [
    ("std::env::var", "inject an environment reader"),
    ("std::env::var_os", "inject an environment reader"),
    ("std::env::vars", "inject an environment reader"),
    ("std::env::vars_os", "inject an environment reader"),
    ("std::env::set_var", "use a stub environment in tests"),
    ("std::env::remove_var", "use a stub environment in tests"),
];

/// Return the repository root, which is where `clippy.toml` lives.
fn repository_root() -> PathBuf { PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("..").join("..") }

/// Return the fixture package manifest.
fn fixture_manifest() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("environment-policy-violation")
        .join("Cargo.toml")
}

/// Lint the fixture with the repository's Clippy configuration.
///
/// The child's environment is composed explicitly: the target directory is
/// redirected so the run leaves nothing behind, `RUSTFLAGS` is emptied so the
/// caller's flags cannot change the diagnostics being asserted, and colour is
/// disabled so the caller's `CARGO_TERM_COLOR` cannot interleave escape
/// sequences with the text the assertions match.
fn lint_fixture(target_directory: &TempDir) -> Result<String, String> {
    let output = Command::new(env!("CARGO"))
        .arg("clippy")
        .arg("--quiet")
        .arg("--manifest-path")
        .arg(fixture_manifest())
        .arg("--")
        .arg("-D")
        .arg("clippy::disallowed_methods")
        .env("CLIPPY_CONF_DIR", repository_root())
        .env("CARGO_TARGET_DIR", target_directory.path())
        .env("CARGO_BUILD_BUILD_DIR", target_directory.path().join("build"))
        .env("RUSTFLAGS", "")
        .env("CARGO_TERM_COLOR", "never")
        .output()
        .map_err(|error| format!("cargo clippy should run: {error}"))?;

    if output.status.success() {
        return Err("cargo clippy should have rejected the fixture".to_owned());
    }
    String::from_utf8(output.stderr).map_err(|error| format!("stderr should be UTF-8: {error}"))
}

/// Return the diagnostic block Clippy emitted for one method.
///
/// A block runs from the `error:` header naming the method to the next
/// `error:` header. Scoping the search this way is what makes the remedy
/// assertion mean anything: three methods share each remedy string, so a
/// search across the whole output would pass even when a method carried the
/// wrong reason.
fn diagnostic_block<'output>(diagnostics: &'output str, method: &str) -> Option<&'output str> {
    let header = format!("error: use of a disallowed method `{method}`\n");
    let block = diagnostics.get(diagnostics.find(&header)?..)?;
    let end = block
        .get(header.len()..)
        .and_then(|tail| tail.find("\nerror: "))
        .map_or(block.len(), |offset| header.len() + offset);
    block.get(..end)
}

/// Scenario: a contributor calls a prohibited method and runs Clippy.
///
/// Invariant: every one of the six methods is reported as an error, and each
/// diagnostic carries the remedy `clippy.toml` pairs with that method, so the
/// contributor is told what to do instead of only what not to do.
#[test]
fn every_prohibited_environment_method_is_reported_with_its_remedy() {
    let target_directory = TempDir::new().expect("temporary target directory should be created");
    let diagnostics = lint_fixture(&target_directory).expect("the fixture should fail Clippy");

    for (method, remedy) in REPORTED_METHODS {
        let block = diagnostic_block(&diagnostics, method);
        assert!(block.is_some(), "Clippy should report {method}; diagnostics were:\n{diagnostics}");
        assert!(
            block.is_some_and(|reported| reported.contains(remedy)),
            "the {method} diagnostic should carry the remedy {remedy:?}; its block was:\n{block:?}"
        );
    }
}
