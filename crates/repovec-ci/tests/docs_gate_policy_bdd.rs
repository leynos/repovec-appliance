//! Behavioural coverage for docs-gate classification.

use std::collections::BTreeSet;

use repovec_ci::{DocsGatePlan, DocsGateReason, MermaidDetection, evaluate_docs_gate_with};
use rstest::fixture;
use rstest_bdd_macros::{given, scenario, then, when};

#[derive(Default)]
struct PolicyWorld {
    changed_files: Vec<String>,
    mermaid_files: BTreeSet<String>,
    unreadable_files: BTreeSet<String>,
    plan: Option<DocsGatePlan>,
}

#[fixture]
fn world() -> PolicyWorld {
    let changed_files = Vec::new();
    let mermaid_files = BTreeSet::new();
    let unreadable_files = BTreeSet::new();
    PolicyWorld { changed_files, mermaid_files, unreadable_files, plan: None }
}

#[given("the changed file list contains {path}")]
fn changed_file(world: &mut PolicyWorld, path: String) { world.changed_files.push(path); }

#[given("the changed file list is unavailable")]
fn missing_changed_files(world: &mut PolicyWorld) { world.changed_files.clear(); }

#[given("the Mermaid-bearing file is {path}")]
fn mermaid_file(world: &mut PolicyWorld, path: String) { let _ = world.mermaid_files.insert(path); }

#[given("the unreadable Markdown file is {path}")]
fn unreadable_file(world: &mut PolicyWorld, path: String) {
    let _ = world.unreadable_files.insert(path);
}

#[when("the docs gate policy is evaluated")]
fn evaluate(world: &mut PolicyWorld) {
    world.plan = Some(evaluate_docs_gate_with(
        world.changed_files.iter().map(std::string::String::as_str),
        |path| {
            if world.unreadable_files.contains(path) {
                return MermaidDetection::Unknown;
            }

            if world.mermaid_files.contains(path) {
                MermaidDetection::Present
            } else {
                MermaidDetection::Absent
            }
        },
    ));
}

#[then("the docs gate runs")]
fn docs_gate_runs(world: &PolicyWorld) {
    assert!(plan(world).should_run());
}

#[then("the docs gate is skipped")]
fn docs_gate_skips(world: &PolicyWorld) {
    assert!(!plan(world).should_run());
}

#[then("Mermaid validation is required")]
fn nixie_runs(world: &PolicyWorld) {
    assert!(plan(world).nixie_required());
}

#[then("Mermaid validation is skipped")]
fn nixie_skips(world: &PolicyWorld) {
    assert!(!plan(world).nixie_required());
}

#[then(expr = "the conservative fallback count is {count}")]
fn fallback_count(world: &PolicyWorld, count: usize) {
    assert_eq!(plan(world).conservative_fallback_files().len(), count);
}

#[then(expr = "the conservative fallback list contains {path}")]
fn fallback_list_contains(world: &PolicyWorld, path: String) {
    assert!(plan(world).conservative_fallback_files().iter().any(|item| item == &path));
}

#[then(expr = "the docs gate reason is {reason}")]
fn docs_gate_reason(world: &PolicyWorld, reason: String) {
    assert_eq!(plan(world).reason().as_str(), reason);
}

#[then(expr = "the docs gate matches {path}")]
fn docs_gate_matches(world: &PolicyWorld, path: String) {
    assert!(plan(world).matched_files().iter().any(|item| item == &path));
}

const fn plan(world: &PolicyWorld) -> &DocsGatePlan {
    match &world.plan {
        Some(plan) => plan,
        None => panic!("docs-gate policy should be evaluated before assertions"),
    }
}

#[scenario(
    path = "tests/features/docs_gate.feature",
    name = "Markdown-only changes trigger the docs gate"
)]
fn markdown_changes_trigger_docs_gate(world: PolicyWorld) {
    assert_eq!(plan(&world).reason(), DocsGateReason::DocumentationChanged);
}

#[scenario(
    path = "tests/features/docs_gate.feature",
    name = "Code-only changes skip the docs gate"
)]
fn code_changes_skip_docs_gate(world: PolicyWorld) {
    assert_eq!(plan(&world).reason(), DocsGateReason::NoDocumentationChanges);
}

#[scenario(
    path = "tests/features/docs_gate.feature",
    name = "Mixed changes still trigger the docs gate"
)]
fn mixed_changes_trigger_docs_gate(world: PolicyWorld) {
    assert!(plan(&world).matched_files().iter().any(|item| item == "README.md"));
}

#[scenario(
    path = "tests/features/docs_gate.feature",
    name = "Documentation tooling changes trigger the docs gate conservatively"
)]
fn docs_tooling_changes_trigger_docs_gate(world: PolicyWorld) {
    assert!(plan(&world).nixie_required());
}

#[scenario(
    path = "tests/features/docs_gate.feature",
    name = "Mermaid-bearing docs changes require nixie"
)]
fn mermaid_docs_require_nixie(world: PolicyWorld) {
    assert!(plan(&world).nixie_required());
}

#[scenario(
    path = "tests/features/docs_gate.feature",
    name = "Unreadable changed Markdown triggers conservative fallback"
)]
fn unreadable_markdown_triggers_fallback(world: PolicyWorld) {
    assert!(plan(&world).nixie_required());
}

#[scenario(
    path = "tests/features/docs_gate.feature",
    name = "Missing changed-file input runs the docs gate conservatively"
)]
fn missing_input_runs_docs_gate(world: PolicyWorld) {
    assert_eq!(plan(&world).reason(), DocsGateReason::MissingChangedFiles);
}
