//! CI policy helpers for repository automation and merge gating.

use camino::Utf8Path;
use cap_std::fs_utf8::Dir;

const DOCS_TOOLING_CONFIG_PATHS: &[&str] = &[".markdownlint-cli2.jsonc"];

/// The reason the documentation gate should or should not run.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DocsGateReason {
    /// At least one documentation input changed.
    DocumentationChanged,
    /// No changed-file list was available, so the safe fallback is to run.
    MissingChangedFiles,
    /// The changed-file list was available and did not include docs inputs.
    NoDocumentationChanges,
}

impl DocsGateReason {
    /// Returns the stable identifier used by workflow logging and ruleset docs.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::DocumentationChanged => "documentation-changed",
            Self::MissingChangedFiles => "missing-changed-files",
            Self::NoDocumentationChanges => "no-documentation-changes",
        }
    }
}

/// The computed plan for the documentation gate.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DocsGatePlan {
    matched_files: Vec<String>,
    conservative_fallback_files: Vec<String>,
    docs_gate_required: bool,
    nixie_required: bool,
    reason: DocsGateReason,
}

impl DocsGatePlan {
    /// Creates a plan for required documentation validation.
    #[must_use]
    const fn required(
        matched_files: Vec<String>,
        conservative_fallback_files: Vec<String>,
        nixie_required: bool,
    ) -> Self {
        Self {
            matched_files,
            conservative_fallback_files,
            docs_gate_required: true,
            nixie_required,
            reason: DocsGateReason::DocumentationChanged,
        }
    }

    /// Creates a plan for the conservative missing-input fallback.
    #[must_use]
    const fn missing_changed_files() -> Self {
        Self {
            matched_files: Vec::new(),
            conservative_fallback_files: Vec::new(),
            docs_gate_required: true,
            nixie_required: true,
            reason: DocsGateReason::MissingChangedFiles,
        }
    }

    /// Creates a plan for skipping documentation validation.
    #[must_use]
    const fn no_documentation_changes() -> Self {
        Self {
            matched_files: Vec::new(),
            conservative_fallback_files: Vec::new(),
            docs_gate_required: false,
            nixie_required: false,
            reason: DocsGateReason::NoDocumentationChanges,
        }
    }

    /// Returns whether the documentation gate should run.
    ///
    /// `should_run()` is a semantic alias for
    /// [`docs_gate_required()`](Self::docs_gate_required) and exists for
    /// convenience and compatibility with external callers. Both methods return
    /// the underlying `self.docs_gate_required` field.
    #[must_use]
    pub const fn should_run(&self) -> bool { self.docs_gate_required }

    /// Returns whether the documentation gate should run.
    ///
    /// This is the canonical accessor for the underlying
    /// `self.docs_gate_required` field. [`should_run()`](Self::should_run) is
    /// provided as a semantic alias for convenience and compatibility.
    #[must_use]
    pub const fn docs_gate_required(&self) -> bool { self.docs_gate_required }

    /// Returns whether Mermaid validation should run.
    #[must_use]
    pub const fn nixie_required(&self) -> bool { self.nixie_required }

    /// Returns the reason for the decision.
    #[must_use]
    pub const fn reason(&self) -> DocsGateReason { self.reason }

    /// Returns the docs inputs that triggered the documentation gate.
    #[must_use]
    pub fn matched_files(&self) -> &[String] { &self.matched_files }

    /// Returns files that triggered a conservative Mermaid-validation fallback.
    #[must_use]
    pub fn conservative_fallback_files(&self) -> &[String] { &self.conservative_fallback_files }
}

/// The result of checking whether a documentation input contains Mermaid.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MermaidDetection {
    /// The file contents contain a Mermaid code fence.
    Present,
    /// The file contents were read successfully and contain no Mermaid code fence.
    Absent,
    /// The file could not be read, so Mermaid validation must run conservatively.
    Unknown,
}

impl MermaidDetection {
    const fn requires_nixie(self) -> bool { !matches!(self, Self::Absent) }

    const fn is_unknown(self) -> bool { matches!(self, Self::Unknown) }
}

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
/// let root = cap_std::fs_utf8::Dir::open_ambient_dir(
///     ".",
///     cap_std::ambient_authority(),
/// )
/// .expect("current directory should be available");
/// let plan = evaluate_docs_gate_in(&root, ["docs/roadmap.md"]);
///
/// assert!(plan.should_run());
/// assert_eq!(plan.reason(), DocsGateReason::DocumentationChanged);
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
/// assert!(plan.should_run());
/// assert!(!plan.nixie_required());
/// assert_eq!(plan.reason(), DocsGateReason::DocumentationChanged);
/// assert_eq!(plan.matched_files(), &["docs/roadmap.md".to_string()]);
/// ```
///
/// ```
/// use repovec_ci::{DocsGateReason, evaluate_docs_gate_with};
///
/// let plan = evaluate_docs_gate_with(
///     ["crates/repovec-core/src/lib.rs"],
///     |_path| repovec_ci::MermaidDetection::Absent,
/// );
///
/// assert!(!plan.should_run());
/// assert_eq!(plan.reason(), DocsGateReason::NoDocumentationChanges);
/// ```
///
/// ```
/// use repovec_ci::{DocsGateReason, MermaidDetection, evaluate_docs_gate_with};
///
/// let plan =
///     evaluate_docs_gate_with(std::iter::empty::<&str>(), |_path| MermaidDetection::Absent);
///
/// assert!(plan.should_run());
/// assert!(plan.nixie_required());
/// assert_eq!(plan.reason(), DocsGateReason::MissingChangedFiles);
/// ```
///
/// ```
/// use repovec_ci::{DocsGateReason, MermaidDetection, evaluate_docs_gate_with};
///
/// let plan =
///     evaluate_docs_gate_with([".markdownlint-cli2.jsonc"], |_path| MermaidDetection::Absent);
///
/// assert!(plan.should_run());
/// assert!(plan.nixie_required());
/// assert_eq!(plan.reason(), DocsGateReason::DocumentationChanged);
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
        let mut conservative_fallback_files = Vec::new();
        let nixie_required = matched_files.iter().any(|path| {
            if is_docs_tooling_config_path(path) {
                return true;
            }

            if !is_markdown_path(path) {
                return false;
            }

            let detection = path_contains_mermaid(path);
            if detection.is_unknown() {
                conservative_fallback_files.push(path.clone());
            }

            detection.requires_nixie()
        });

        DocsGatePlan::required(matched_files, conservative_fallback_files, nixie_required)
    }
}

fn normalize_path(path: &str) -> Option<String> {
    let trimmed = path.trim();
    if trimmed.is_empty() {
        return None;
    }

    Some(trimmed.strip_prefix("./").map_or_else(|| trimmed.to_owned(), str::to_owned))
}

fn is_documentation_input(path: &str) -> bool {
    is_markdown_path(path) || is_docs_tooling_config_path(path)
}

fn is_markdown_path(path: &str) -> bool {
    Utf8Path::new(path).extension().is_some_and(|extension| {
        extension.eq_ignore_ascii_case("md")
            || extension.eq_ignore_ascii_case("markdown")
            || extension.eq_ignore_ascii_case("mdx")
    })
}

fn is_docs_tooling_config_path(path: &str) -> bool {
    DOCS_TOOLING_CONFIG_PATHS.iter().any(|known_path| path.eq_ignore_ascii_case(known_path))
}

fn path_contains_mermaid(root: &Dir, path: &str) -> MermaidDetection {
    root.read_to_string(Utf8Path::new(path)).map_or(MermaidDetection::Unknown, |contents| {
        if contents.contains("```mermaid") {
            MermaidDetection::Present
        } else {
            MermaidDetection::Absent
        }
    })
}

#[cfg(test)]
mod tests {
    //! Unit coverage for docs-gate classification.

    use rstest::rstest;

    use super::{DocsGateReason, MermaidDetection, evaluate_docs_gate_with};

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
                if path == "README.md" {
                    MermaidDetection::Present
                } else {
                    MermaidDetection::Absent
                }
            },
        );

        assert!(plan.should_run());
        assert!(plan.nixie_required());
        assert_eq!(plan.reason(), DocsGateReason::DocumentationChanged);
        assert_eq!(plan.matched_files(), &["docs/roadmap.md".to_owned(), "README.md".to_owned()]);
    }

    #[test]
    fn mermaid_docs_request_nixie() {
        let plan =
            evaluate_docs_gate_with(["docs/users-guide.md"], |_path| MermaidDetection::Present);

        assert!(plan.docs_gate_required());
        assert!(plan.nixie_required());
        assert_eq!(plan.reason(), DocsGateReason::DocumentationChanged);
    }

    #[test]
    fn unreadable_markdown_requests_nixie_conservatively() {
        let plan =
            evaluate_docs_gate_with(["docs/users-guide.md"], |_path| MermaidDetection::Unknown);

        assert!(plan.docs_gate_required());
        assert!(plan.nixie_required());
        assert_eq!(plan.reason(), DocsGateReason::DocumentationChanged);
        assert_eq!(plan.conservative_fallback_files(), &["docs/users-guide.md".to_owned()]);
    }
}
