//! Behavioural coverage for the environment-access policy as Clippy applies it.
//!
//! The sibling contract test reads the configuration files. This test runs the
//! lint: it points Clippy at `tests/fixtures/environment-policy-violation`,
//! which calls all six prohibited methods, with `CLIPPY_CONF_DIR` set to the
//! repository root so the checked-in `clippy.toml` is the configuration in
//! force. A configuration that parses but never fires would pass the contract
//! test and fail this one.
//!
//! Mutation proof, recorded 2026-09-06: deleting the `std::env::vars_os` entry
//! from `clippy.toml` leaves that call unreported and fails
//! `every_prohibited_environment_method_is_reported_with_its_remedy`. The
//! fixture is not a workspace member and has no dependencies, so the run needs
//! no network and compiles nothing but itself.

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
/// redirected so the run leaves nothing behind, and `RUSTFLAGS` is emptied so
/// the caller's flags cannot change the diagnostics being asserted.
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
        .output()
        .map_err(|error| format!("cargo clippy should run: {error}"))?;

    if output.status.success() {
        return Err("cargo clippy should have rejected the fixture".to_owned());
    }
    String::from_utf8(output.stderr).map_err(|error| format!("stderr should be UTF-8: {error}"))
}

/// Scenario: a contributor calls a prohibited method and runs Clippy.
///
/// Invariant: every one of the six methods is reported as an error, and each
/// diagnostic carries the remedy `clippy.toml` records, so the contributor is
/// told what to do instead of only what not to do.
#[test]
fn every_prohibited_environment_method_is_reported_with_its_remedy() {
    let target_directory = TempDir::new().expect("temporary target directory should be created");
    let diagnostics = lint_fixture(&target_directory).expect("the fixture should fail Clippy");

    for (method, remedy) in REPORTED_METHODS {
        assert!(
            diagnostics.contains(&format!("use of a disallowed method `{method}`")),
            "Clippy should report {method}; diagnostics were:\n{diagnostics}"
        );
        assert!(
            diagnostics.contains(remedy),
            "the {method} diagnostic should carry the remedy {remedy:?}; diagnostics were:\n{diagnostics}"
        );
    }
}
