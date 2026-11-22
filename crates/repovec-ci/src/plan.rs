//! The documentation-gate plan and the values it is computed from.
//!
//! The plan states whether the documentation checks and Mermaid validation
//! must run and why, so a workflow can log a stable reason and a caller can
//! inspect the files that caused the decision.

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
    ///
    /// # Examples
    ///
    /// ```rust
    /// use repovec_ci::DocsGateReason;
    ///
    /// assert_eq!(
    ///     DocsGateReason::DocumentationChanged.as_str(),
    ///     "documentation-changed",
    ///     "the changed reason should keep its stable identifier"
    /// );
    /// assert_eq!(
    ///     DocsGateReason::MissingChangedFiles.as_str(),
    ///     "missing-changed-files",
    ///     "the fallback reason should keep its stable identifier"
    /// );
    /// assert_eq!(
    ///     DocsGateReason::NoDocumentationChanges.as_str(),
    ///     "no-documentation-changes",
    ///     "the skip reason should keep its stable identifier"
    /// );
    /// ```
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
    /// Documentation paths that matched the input policy.
    matched_files: Vec<String>,
    /// Markdown paths whose contents could not be inspected safely.
    conservative_fallback_files: Vec<String>,
    /// Whether the documentation checks must run for this change.
    docs_gate_required: bool,
    /// Whether Mermaid syntax validation must run for this change.
    nixie_required: bool,
    /// Why the policy selected or skipped the documentation checks.
    reason: DocsGateReason,
}

impl DocsGatePlan {
    /// Creates a plan for required documentation validation.
    #[must_use]
    pub(super) const fn required(
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
    pub(super) const fn missing_changed_files() -> Self {
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
    pub(super) const fn no_documentation_changes() -> Self {
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
    ///
    /// # Examples
    ///
    /// ```rust
    /// use repovec_ci::{MermaidDetection, evaluate_docs_gate_with};
    ///
    /// let plan = evaluate_docs_gate_with(["docs/roadmap.md"], |_path| MermaidDetection::Absent);
    ///
    /// assert!(
    ///     plan.should_run(),
    ///     "a documentation change should require the docs gate"
    /// );
    /// ```
    #[must_use]
    pub const fn should_run(&self) -> bool { self.docs_gate_required }

    /// Returns whether the documentation gate should run.
    ///
    /// This is the canonical accessor for the underlying
    /// `self.docs_gate_required` field. [`should_run()`](Self::should_run) is
    /// provided as a semantic alias for convenience and compatibility.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use repovec_ci::{MermaidDetection, evaluate_docs_gate_with};
    ///
    /// let plan = evaluate_docs_gate_with(["Cargo.toml"], |_path| MermaidDetection::Absent);
    ///
    /// assert!(
    ///     !plan.docs_gate_required(),
    ///     "a non-documentation change should skip the docs gate"
    /// );
    /// ```
    #[must_use]
    pub const fn docs_gate_required(&self) -> bool { self.docs_gate_required }

    /// Returns whether Mermaid validation should run.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use repovec_ci::{MermaidDetection, evaluate_docs_gate_with};
    ///
    /// let plan = evaluate_docs_gate_with(["docs/users-guide.md"], |_path| MermaidDetection::Present);
    ///
    /// assert!(
    ///     plan.nixie_required(),
    ///     "a Mermaid-bearing document should require Nixie"
    /// );
    /// ```
    #[must_use]
    pub const fn nixie_required(&self) -> bool { self.nixie_required }

    /// Returns the reason for the decision.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use repovec_ci::{DocsGateReason, MermaidDetection, evaluate_docs_gate_with};
    ///
    /// let plan =
    ///     evaluate_docs_gate_with(std::iter::empty::<&str>(), |_path| MermaidDetection::Absent);
    ///
    /// assert_eq!(
    ///     plan.reason(),
    ///     DocsGateReason::MissingChangedFiles,
    ///     "an empty file list should select the safe fallback"
    /// );
    /// ```
    #[must_use]
    pub const fn reason(&self) -> DocsGateReason { self.reason }

    /// Returns the docs inputs that triggered the documentation gate.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use repovec_ci::{MermaidDetection, evaluate_docs_gate_with};
    ///
    /// let plan = evaluate_docs_gate_with(
    ///     ["docs/roadmap.md", "crates/repovec-core/src/lib.rs"],
    ///     |_path| MermaidDetection::Absent,
    /// );
    ///
    /// assert_eq!(
    ///     plan.matched_files(),
    ///     &["docs/roadmap.md".to_string()],
    ///     "only the documentation input should be matched"
    /// );
    /// ```
    #[must_use]
    pub fn matched_files(&self) -> &[String] { &self.matched_files }

    /// Returns files that triggered a conservative Mermaid-validation fallback.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use repovec_ci::{MermaidDetection, evaluate_docs_gate_with};
    ///
    /// let plan = evaluate_docs_gate_with(["docs/users-guide.md"], |_path| MermaidDetection::Unknown);
    ///
    /// assert_eq!(
    ///     plan.conservative_fallback_files(),
    ///     &["docs/users-guide.md".to_string()],
    ///     "an unreadable document should be recorded for Mermaid fallback"
    /// );
    /// ```
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
    /// Reports whether this result requires Mermaid validation.
    ///
    /// For example, `Absent` skips Nixie, while `Present` and `Unknown` run it.
    pub(super) const fn requires_nixie(self) -> bool { !matches!(self, Self::Absent) }

    /// Reports whether the content read failed and needs a conservative fallback.
    ///
    /// For example, `Unknown` records the path for fallback reporting.
    pub(super) const fn is_unknown(self) -> bool { matches!(self, Self::Unknown) }
}
