//! Unit coverage for docs-gate classification.

use repovec_ci::{DocsGateReason, MermaidDetection, evaluate_docs_gate_with};
use rstest::rstest;

#[rstest]
#[case("docs/roadmap.md")]
#[case("./README.md")]
#[case("guide.MDX")]
#[case("notes.markdown")]
fn markdown_paths_trigger_the_docs_gate(#[case] changed_file: &str) {
    let plan = evaluate_docs_gate_with([changed_file], |_path| MermaidDetection::Absent);

    assert!(plan.should_run());
    assert!(!plan.nixie_required());
    assert_eq!(plan.reason(), DocsGateReason::DocumentationChanged);
    assert_eq!(plan.matched_files().len(), 1);
}

#[rstest]
#[case(".markdownlint-cli2.jsonc")]
fn docs_tooling_changes_trigger_the_docs_gate(#[case] changed_file: &str) {
    let plan = evaluate_docs_gate_with([changed_file], |_path| MermaidDetection::Absent);

    assert!(plan.should_run());
    assert!(plan.nixie_required());
    assert_eq!(plan.reason(), DocsGateReason::DocumentationChanged);
    assert_eq!(plan.matched_files(), &[changed_file.to_owned()]);
}

#[rstest]
#[case("Cargo.toml")]
#[case("crates/repovec-core/src/lib.rs")]
#[case("assets/logo.svg")]
fn non_markdown_paths_skip_the_docs_gate(#[case] changed_file: &str) {
    let plan = evaluate_docs_gate_with([changed_file], |_path| MermaidDetection::Absent);

    assert!(!plan.should_run());
    assert!(!plan.nixie_required());
    assert_eq!(plan.reason(), DocsGateReason::NoDocumentationChanges);
    assert!(plan.matched_files().is_empty());
}

#[test]
fn empty_input_runs_the_docs_gate_conservatively() {
    let plan =
        evaluate_docs_gate_with(std::iter::empty::<&str>(), |_path| MermaidDetection::Absent);

    assert!(plan.should_run());
    assert!(plan.nixie_required());
    assert_eq!(plan.reason(), DocsGateReason::MissingChangedFiles);
    assert!(plan.matched_files().is_empty());
}

#[test]
fn mixed_input_returns_only_markdown_matches() {
    let plan = evaluate_docs_gate_with(
        ["crates/repovec-core/src/lib.rs", "./docs/roadmap.md", "README.md", ""],
        |path| {
            if path == "README.md" { MermaidDetection::Present } else { MermaidDetection::Absent }
        },
    );

    assert!(plan.should_run());
    assert!(plan.nixie_required());
    assert_eq!(plan.reason(), DocsGateReason::DocumentationChanged);
    assert_eq!(plan.matched_files(), &["docs/roadmap.md".to_owned(), "README.md".to_owned()]);
}

#[test]
fn mermaid_docs_request_nixie() {
    let plan = evaluate_docs_gate_with(["docs/users-guide.md"], |_path| MermaidDetection::Present);

    assert!(plan.docs_gate_required());
    assert!(plan.nixie_required());
    assert_eq!(plan.reason(), DocsGateReason::DocumentationChanged);
}

#[test]
fn unreadable_markdown_requests_nixie_conservatively() {
    let plan = evaluate_docs_gate_with(["docs/users-guide.md"], |_path| MermaidDetection::Unknown);

    assert!(plan.docs_gate_required());
    assert!(plan.nixie_required());
    assert_eq!(plan.reason(), DocsGateReason::DocumentationChanged);
    assert_eq!(plan.conservative_fallback_files(), &["docs/users-guide.md".to_owned()]);
}

#[test]
fn unreadable_markdown_is_recorded_even_after_nixie_is_already_required() {
    let plan =
        evaluate_docs_gate_with([".markdownlint-cli2.jsonc", "docs/users-guide.md"], |path| {
            if path == "docs/users-guide.md" {
                MermaidDetection::Unknown
            } else {
                MermaidDetection::Absent
            }
        });

    assert!(plan.docs_gate_required());
    assert!(plan.nixie_required());
    assert_eq!(plan.conservative_fallback_files(), &["docs/users-guide.md".to_owned()]);
}
