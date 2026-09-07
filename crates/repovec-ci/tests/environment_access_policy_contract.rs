//! Contract coverage for the workspace environment-access policy.
//!
//! The policy has four moving parts, and removing any one of them disarms the
//! gate without failing another check: the root `clippy.toml` must name every
//! prohibited environment method, the workspace lint table must deny
//! `clippy::disallowed_methods`, every member crate must inherit that table,
//! and the lint gate must run Clippy across every workspace target and
//! feature with warnings denied. Each part is asserted against the
//! checked-in file, embedded at compile time, rather than against prose
//! describing it.
//!
//! Mutation proof, recorded 2026-09-06; each mutation was applied on its own
//! and reverted, and each failed exactly the named test:
//!
//! - deleting the `std::env::var_os` entry from `clippy.toml` fails
//!   `clippy_policy_names_every_prohibited_environment_method`;
//! - lowering the workspace `disallowed_methods` level from `deny` to `warn`
//!   fails `workspace_lint_table_denies_disallowed_methods`;
//! - deleting the `[lints] workspace = true` block from
//!   `crates/repovec-core/Cargo.toml` fails
//!   `every_workspace_member_inherits_the_workspace_lint_table` (setting the
//!   key to `false` is not a usable mutation; Cargo rejects the manifest
//!   before any test runs);
//! - dropping `crates/repovectl` from `MEMBER_MANIFESTS` fails
//!   `contract_covers_every_declared_workspace_member`;
//! - dropping `--all-targets` from the Makefile's `CARGO_FLAGS` fails
//!   `lint_gate_covers_every_workspace_target_and_feature`.
//!
//! The workflow side of this policy moved to
//! `tests/workflow_contracts/ci_gate_workflow_test.py`, which parses the
//! workflow instead of searching it: a line-matching check here stayed green
//! when the Lint step was disabled with `if: false`.

use std::collections::BTreeSet;

use toml::Value;

/// Root Clippy configuration that carries the prohibition.
const CLIPPY_POLICY: &str = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/../../clippy.toml"));

/// Workspace manifest that carries the lint table and the member list.
const WORKSPACE_MANIFEST: &str =
    include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/../../Cargo.toml"));

/// Makefile that defines the local and continuous-integration lint gate.
const MAKEFILE: &str = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/../../Makefile"));

/// Prohibited environment methods paired with the remedy Clippy reports.
const REQUIRED_ENVIRONMENT_METHODS: [(&str, &str); 6] = [
    ("std::env::var", "inject an environment reader"),
    ("std::env::var_os", "inject an environment reader"),
    ("std::env::vars", "inject an environment reader"),
    ("std::env::vars_os", "inject an environment reader"),
    ("std::env::set_var", "use a stub environment in tests"),
    ("std::env::remove_var", "use a stub environment in tests"),
];

/// Workspace member manifests, keyed by the path the workspace declares.
const MEMBER_MANIFESTS: [(&str, &str); 7] = [
    ("crates/repovec-ci", include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/Cargo.toml"))),
    (
        "crates/repovec-core",
        include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/../repovec-core/Cargo.toml")),
    ),
    (
        "crates/repovec-mcpd",
        include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/../repovec-mcpd/Cargo.toml")),
    ),
    (
        "crates/repovec-test-helpers",
        include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/../repovec-test-helpers/Cargo.toml")),
    ),
    (
        "crates/repovec-tui",
        include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/../repovec-tui/Cargo.toml")),
    ),
    (
        "crates/repovecd",
        include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/../repovecd/Cargo.toml")),
    ),
    (
        "crates/repovectl",
        include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/../repovectl/Cargo.toml")),
    ),
];

/// Makefile lines that together scope Clippy to the whole workspace.
const REQUIRED_MAKEFILE_LINES: [&str; 4] = [
    "BASE_RUST_FLAGS ?= -D warnings",
    "CARGO_FLAGS ?= --all-targets --all-features",
    "CLIPPY_FLAGS ?= $(CARGO_FLAGS) -- $(EFFECTIVE_RUST_FLAGS)",
    "$(CARGO) clippy --workspace $(CLIPPY_FLAGS)",
];

/// Parse an embedded manifest, naming the file in any parse failure.
fn parse_toml(label: &str, contents: &str) -> Result<Value, String> {
    toml::from_str(contents).map_err(|error| format!("{label} should parse as TOML: {error}"))
}

/// Collect the `path` and `reason` of every `disallowed-methods` entry.
fn disallowed_entries(policy: &Value) -> Result<Vec<(String, String)>, String> {
    let methods = policy
        .get("disallowed-methods")
        .and_then(Value::as_array)
        .ok_or_else(|| "clippy.toml should declare a disallowed-methods array".to_owned())?;
    methods
        .iter()
        .map(|entry| {
            let path = entry
                .get("path")
                .and_then(Value::as_str)
                .ok_or_else(|| format!("disallowed-methods entry {entry} should carry a path"))?;
            let reason = entry.get("reason").and_then(Value::as_str).unwrap_or_default();
            Ok((path.to_owned(), reason.to_owned()))
        })
        .collect()
}

/// Read the Clippy lint level the workspace lint table sets for one lint.
fn workspace_clippy_lint_level<'manifest>(
    manifest: &'manifest Value,
    lint: &str,
) -> Option<&'manifest str> {
    manifest
        .get("workspace")
        .and_then(|workspace| workspace.get("lints"))
        .and_then(|lints| lints.get("clippy"))
        .and_then(|clippy| clippy.get(lint))
        .and_then(Value::as_str)
}

/// Report whether a file carries the given line, ignoring indentation.
fn contains_line(contents: &str, line: &str) -> bool {
    contents.lines().any(|candidate| candidate.trim() == line)
}

/// Scenario: a contributor's Clippy run reads the checked-in configuration.
///
/// Invariant: every prohibited environment method is named, each with the
/// remedy the diagnostic must show, so no method can be quietly dropped.
#[test]
fn clippy_policy_names_every_prohibited_environment_method() {
    let policy = parse_toml("clippy.toml", CLIPPY_POLICY).expect("clippy.toml should parse");
    let entries = disallowed_entries(&policy).expect("clippy.toml should list disallowed methods");

    for (path, reason) in REQUIRED_ENVIRONMENT_METHODS {
        let matched = entries
            .iter()
            .any(|(entry_path, entry_reason)| entry_path == path && entry_reason == reason);
        assert!(
            matched,
            "clippy.toml must disallow {path} with reason {reason:?}; found {entries:?}"
        );
    }
}

/// Scenario: the workspace lint table is the only place the prohibition is
/// promoted from a Clippy suggestion to a build failure.
///
/// Invariant: `clippy::disallowed_methods` is denied, not warned.
#[test]
fn workspace_lint_table_denies_disallowed_methods() {
    let manifest =
        parse_toml("Cargo.toml", WORKSPACE_MANIFEST).expect("workspace manifest should parse");

    assert_eq!(
        workspace_clippy_lint_level(&manifest, "disallowed_methods"),
        Some("deny"),
        "the workspace lint table must deny clippy::disallowed_methods"
    );
}

/// Scenario: a crate added to the workspace after this policy landed.
///
/// Invariant: the member list and this contract's manifest table agree, so a
/// new crate cannot join the workspace without being checked for inheritance.
#[test]
fn contract_covers_every_declared_workspace_member() {
    let manifest =
        parse_toml("Cargo.toml", WORKSPACE_MANIFEST).expect("workspace manifest should parse");
    let declared: BTreeSet<&str> = manifest
        .get("workspace")
        .and_then(|workspace| workspace.get("members"))
        .and_then(Value::as_array)
        .expect("workspace manifest should list members")
        .iter()
        .filter_map(Value::as_str)
        .collect();
    let covered: BTreeSet<&str> = MEMBER_MANIFESTS.iter().map(|(member, _)| *member).collect();

    assert_eq!(declared, covered, "every workspace member must be covered by this contract");
}

/// Scenario: a member crate replaces the inherited lint table with its own.
///
/// Invariant: every member inherits the workspace lints, so the deny reaches
/// every package rather than only those that opted in.
#[test]
fn every_workspace_member_inherits_the_workspace_lint_table() {
    for (member, contents) in MEMBER_MANIFESTS {
        let manifest = parse_toml(member, contents).expect("member manifest should parse");
        let inherits =
            manifest.get("lints").and_then(|lints| lints.get("workspace")).and_then(Value::as_bool);

        assert_eq!(inherits, Some(true), "{member} must inherit the workspace lint table");
    }
}

/// Scenario: the lint gate is narrowed to fewer targets or features.
///
/// Invariant: Clippy runs over the whole workspace, every target kind, and
/// every feature, with warnings denied, so tests are covered as well as
/// production code.
#[test]
fn lint_gate_covers_every_workspace_target_and_feature() {
    for line in REQUIRED_MAKEFILE_LINES {
        assert!(contains_line(MAKEFILE, line), "the Makefile must contain the line {line:?}");
    }
}
