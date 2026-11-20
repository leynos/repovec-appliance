//! Classification of the documentation inputs a change touched.
//!
//! The scan folds each matched path into the two decisions the plan needs:
//! which Markdown files need conservative Mermaid validation because their
//! contents could not be read, and whether any input requires validation.

use super::{
    MermaidDetection,
    documentation_inputs::{is_docs_tooling_config_path, is_markdown_path},
};

/// Outcome of classifying the documentation files a change touched.
pub(super) struct DocsScan {
    /// Markdown files whose Mermaid content could not be determined.
    pub(super) fallback_files: Vec<String>,
    /// Whether any changed file requires Mermaid validation.
    pub(super) nixie_required: bool,
}

/// Classifies each matched documentation input for Mermaid validation.
///
/// Tooling configuration paths force validation without inspection, because a
/// change to them can alter how diagrams are validated. Non-Markdown inputs
/// cannot contain Mermaid. A Markdown file whose detector reports an unknown
/// outcome is retained conservatively so a later stage can decide.
pub(super) fn scan_matched_files<F>(
    matched_files: &[String],
    path_contains_mermaid: &mut F,
) -> DocsScan
where
    F: FnMut(&str) -> MermaidDetection,
{
    let mut scan = DocsScan {
        fallback_files: Vec::new(),
        nixie_required: false,
    };

    for path in matched_files {
        scan.observe(path, path_contains_mermaid);
    }

    scan
}

impl DocsScan {
    /// Folds one matched path into the scan outcome.
    fn observe<F>(&mut self, path: &str, path_contains_mermaid: &mut F)
    where
        F: FnMut(&str) -> MermaidDetection,
    {
        if is_docs_tooling_config_path(path) {
            self.nixie_required = true;
            return;
        }

        if !is_markdown_path(path) {
            return;
        }

        let detection = path_contains_mermaid(path);
        if detection.is_unknown() {
            self.fallback_files.push(path.to_owned());
        }

        if detection.requires_nixie() {
            self.nixie_required = true;
        }
    }
}
