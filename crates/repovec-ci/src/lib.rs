//! CI policy helpers for repository automation and merge gating.

use std::path::Path;

/// The reason the documentation gate should or should not run.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DocsGateReason {
    /// At least one Markdown file changed.
    MarkdownChanged,
    /// No changed-file list was available, so the safe fallback is to run.
    MissingChangedFiles,
    /// The changed-file list was available and did not include Markdown files.
    NoMarkdownChanges,
}

impl DocsGateReason {
    /// Returns the stable identifier used by workflow logging and ruleset docs.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::MarkdownChanged => "markdown-changed",
            Self::MissingChangedFiles => "missing-changed-files",
            Self::NoMarkdownChanges => "no-markdown-changes",
        }
    }
}

/// The computed plan for the documentation gate.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DocsGatePlan {
    matched_files: Vec<String>,
    reason: DocsGateReason,
}

impl DocsGatePlan {
    /// Creates a plan from an explicit reason and the matched Markdown files.
    #[must_use]
    pub const fn new(matched_files: Vec<String>, reason: DocsGateReason) -> Self {
        Self { matched_files, reason }
    }

    /// Returns whether the documentation gate should run.
    #[must_use]
    pub const fn should_run(&self) -> bool {
        !matches!(self.reason, DocsGateReason::NoMarkdownChanges)
    }

    /// Returns the reason for the decision.
    #[must_use]
    pub const fn reason(&self) -> DocsGateReason { self.reason }

    /// Returns the Markdown files that triggered the documentation gate.
    #[must_use]
    pub fn matched_files(&self) -> &[String] { &self.matched_files }
}

/// Evaluates whether Markdown validation should run for the provided file list.
///
/// The policy is intentionally conservative: when no changed-file list is
/// available, the documentation gate runs rather than risking a skipped lint.
///
/// # Examples
///
/// ```
/// use repovec_ci::{DocsGateReason, evaluate_docs_gate};
///
/// let plan = evaluate_docs_gate(["docs/roadmap.md", "crates/repovec-core/src/lib.rs"]);
///
/// assert!(plan.should_run());
/// assert_eq!(plan.reason(), DocsGateReason::MarkdownChanged);
/// assert_eq!(plan.matched_files(), &["docs/roadmap.md".to_string()]);
/// ```
///
/// ```
/// use repovec_ci::{DocsGateReason, evaluate_docs_gate};
///
/// let plan = evaluate_docs_gate(["crates/repovec-core/src/lib.rs"]);
///
/// assert!(!plan.should_run());
/// assert_eq!(plan.reason(), DocsGateReason::NoMarkdownChanges);
/// ```
///
/// ```
/// use repovec_ci::{DocsGateReason, evaluate_docs_gate};
///
/// let plan = evaluate_docs_gate(std::iter::empty::<&str>());
///
/// assert!(plan.should_run());
/// assert_eq!(plan.reason(), DocsGateReason::MissingChangedFiles);
/// ```
#[must_use]
pub fn evaluate_docs_gate<I, S>(changed_files: I) -> DocsGatePlan
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    let normalized_files = changed_files
        .into_iter()
        .filter_map(|path| normalize_path(path.as_ref()))
        .collect::<Vec<_>>();

    if normalized_files.is_empty() {
        return DocsGatePlan::new(Vec::new(), DocsGateReason::MissingChangedFiles);
    }

    let matched_files =
        normalized_files.into_iter().filter(|path| is_markdown_path(path)).collect::<Vec<_>>();

    if matched_files.is_empty() {
        DocsGatePlan::new(Vec::new(), DocsGateReason::NoMarkdownChanges)
    } else {
        DocsGatePlan::new(matched_files, DocsGateReason::MarkdownChanged)
    }
}

fn normalize_path(path: &str) -> Option<String> {
    let trimmed = path.trim();
    if trimmed.is_empty() {
        return None;
    }

    Some(trimmed.strip_prefix("./").map_or_else(|| trimmed.to_owned(), str::to_owned))
}

fn is_markdown_path(path: &str) -> bool {
    Path::new(path).extension().and_then(|extension| extension.to_str()).is_some_and(|extension| {
        extension.eq_ignore_ascii_case("md")
            || extension.eq_ignore_ascii_case("markdown")
            || extension.eq_ignore_ascii_case("mdx")
    })
}

#[cfg(test)]
mod tests {
    //! Unit coverage for docs-gate classification.

    use rstest::rstest;

    use super::{DocsGateReason, evaluate_docs_gate};

    #[rstest]
    #[case("docs/roadmap.md")]
    #[case("./README.md")]
    #[case("guide.MDX")]
    #[case("notes.markdown")]
    fn markdown_paths_trigger_the_docs_gate(#[case] changed_file: &str) {
        let plan = evaluate_docs_gate([changed_file]);

        assert!(plan.should_run());
        assert_eq!(plan.reason(), DocsGateReason::MarkdownChanged);
        assert_eq!(plan.matched_files().len(), 1);
    }

    #[rstest]
    #[case("Cargo.toml")]
    #[case("crates/repovec-core/src/lib.rs")]
    #[case("assets/logo.svg")]
    fn non_markdown_paths_skip_the_docs_gate(#[case] changed_file: &str) {
        let plan = evaluate_docs_gate([changed_file]);

        assert!(!plan.should_run());
        assert_eq!(plan.reason(), DocsGateReason::NoMarkdownChanges);
        assert!(plan.matched_files().is_empty());
    }

    #[test]
    fn empty_input_runs_the_docs_gate_conservatively() {
        let plan = evaluate_docs_gate(std::iter::empty::<&str>());

        assert!(plan.should_run());
        assert_eq!(plan.reason(), DocsGateReason::MissingChangedFiles);
        assert!(plan.matched_files().is_empty());
    }

    #[test]
    fn mixed_input_returns_only_markdown_matches() {
        let plan = evaluate_docs_gate([
            "crates/repovec-core/src/lib.rs",
            "./docs/roadmap.md",
            "README.md",
            "",
        ]);

        assert!(plan.should_run());
        assert_eq!(plan.reason(), DocsGateReason::MarkdownChanged);
        assert_eq!(plan.matched_files(), &["docs/roadmap.md".to_owned(), "README.md".to_owned()]);
    }
}
