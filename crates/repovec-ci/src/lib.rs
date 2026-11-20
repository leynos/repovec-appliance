//! CI policy helpers for repository automation and merge gating.

use cap_std::fs_utf8::Dir;

mod documentation_inputs;
mod plan;
mod scan;

use documentation_inputs::{is_documentation_input, normalize_path, path_contains_mermaid};
pub use plan::{DocsGatePlan, DocsGateReason, MermaidDetection};
use scan::scan_matched_files;

/// Evaluates whether documentation validation should run for the provided file
/// list.
///
/// The policy is intentionally conservative: when no changed-file list is
/// available, the documentation gate runs rather than risking a skipped lint.
///
/// # Examples
///
/// ```
/// use repovec_ci::{DocsGateReason, MermaidDetection, evaluate_docs_gate_in};
///
/// let root = cap_std::fs_utf8::Dir::open_ambient_dir(".", cap_std::ambient_authority())
///     .expect("current directory should be available");
/// let plan = evaluate_docs_gate_in(&root, ["docs/roadmap.md"]);
///
/// assert!(
///     plan.should_run(),
///     "a documentation change should require the docs gate"
/// );
/// assert_eq!(
///     plan.reason(),
///     DocsGateReason::DocumentationChanged,
///     "a documentation change should report its reason"
/// );
/// ```
///
/// ```
/// use repovec_ci::{DocsGateReason, MermaidDetection, evaluate_docs_gate_with};
///
/// let plan = evaluate_docs_gate_with(
///     ["docs/roadmap.md", "crates/repovec-core/src/lib.rs"],
///     |_path| MermaidDetection::Absent,
/// );
///
/// assert!(
///     plan.should_run(),
///     "a documentation change should require the docs gate"
/// );
/// assert!(
///     !plan.nixie_required(),
///     "a document without Mermaid should skip Nixie"
/// );
/// assert_eq!(
///     plan.reason(),
///     DocsGateReason::DocumentationChanged,
///     "a documentation change should report its reason"
/// );
/// assert_eq!(
///     plan.matched_files(),
///     &["docs/roadmap.md".to_string()],
///     "only the documentation input should be matched"
/// );
/// ```
///
/// ```
/// use repovec_ci::{DocsGateReason, evaluate_docs_gate_with};
///
/// let plan = evaluate_docs_gate_with(["crates/repovec-core/src/lib.rs"], |_path| {
///     repovec_ci::MermaidDetection::Absent
/// });
///
/// assert!(
///     !plan.should_run(),
///     "a non-documentation change should skip the docs gate"
/// );
/// assert_eq!(
///     plan.reason(),
///     DocsGateReason::NoDocumentationChanges,
///     "the skipped gate should report that no documentation changed"
/// );
/// ```
///
/// ```
/// use repovec_ci::{DocsGateReason, MermaidDetection, evaluate_docs_gate_with};
///
/// let plan =
///     evaluate_docs_gate_with(std::iter::empty::<&str>(), |_path| MermaidDetection::Absent);
///
/// assert!(
///     plan.should_run(),
///     "a missing file list should run the docs gate conservatively"
/// );
/// assert!(
///     plan.nixie_required(),
///     "a missing file list should also run Mermaid validation"
/// );
/// assert_eq!(
///     plan.reason(),
///     DocsGateReason::MissingChangedFiles,
///     "the fallback should report its reason"
/// );
/// ```
///
/// ```
/// use repovec_ci::{DocsGateReason, MermaidDetection, evaluate_docs_gate_with};
///
/// let plan = evaluate_docs_gate_with([".markdownlint-cli2.jsonc"], |_path| {
///     MermaidDetection::Absent
/// });
///
/// assert!(
///     plan.should_run(),
///     "a tooling-config change should run the docs gate"
/// );
/// assert!(
///     plan.nixie_required(),
///     "a tooling-config change should also run Mermaid validation"
/// );
/// assert_eq!(
///     plan.reason(),
///     DocsGateReason::DocumentationChanged,
///     "a tooling-config change should report its reason"
/// );
/// ```
#[must_use]
pub fn evaluate_docs_gate_in<I, S>(root: &Dir, changed_files: I) -> DocsGatePlan
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    evaluate_docs_gate_with(changed_files, |path| path_contains_mermaid(root, path))
}

/// Evaluates the docs gate policy with an injected Mermaid detector.
///
/// # Examples
///
/// ```rust
/// use repovec_ci::{DocsGateReason, MermaidDetection, evaluate_docs_gate_with};
///
/// let plan = evaluate_docs_gate_with(["README.md", ".markdownlint-cli2.jsonc"], |path| {
///     if path == "README.md" {
///         MermaidDetection::Present
///     } else {
///         MermaidDetection::Absent
///     }
/// });
///
/// assert!(
///     plan.docs_gate_required(),
///     "both inputs are documentation or tooling configuration"
/// );
/// assert!(
///     plan.nixie_required(),
///     "one matched document contains Mermaid"
/// );
/// assert_eq!(
///     plan.reason(),
///     DocsGateReason::DocumentationChanged,
///     "a documentation change should report its reason"
/// );
/// assert_eq!(
///     plan.matched_files(),
///     &[
///         "README.md".to_string(),
///         ".markdownlint-cli2.jsonc".to_string()
///     ],
///     "both documentation inputs should be matched"
/// );
/// ```
#[must_use]
pub fn evaluate_docs_gate_with<I, S, F>(
    changed_files: I,
    mut path_contains_mermaid: F,
) -> DocsGatePlan
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
    F: FnMut(&str) -> MermaidDetection,
{
    let normalized_files = changed_files
        .into_iter()
        .filter_map(|path| normalize_path(path.as_ref()))
        .collect::<Vec<_>>();

    if normalized_files.is_empty() {
        return DocsGatePlan::missing_changed_files();
    }

    let matched_files = normalized_files
        .into_iter()
        .filter(|path| is_documentation_input(path))
        .collect::<Vec<_>>();

    if matched_files.is_empty() {
        DocsGatePlan::no_documentation_changes()
    } else {
        let scan = scan_matched_files(&matched_files, &mut path_contains_mermaid);
        DocsGatePlan::required(matched_files, scan.fallback_files, scan.nixie_required)
    }
}
