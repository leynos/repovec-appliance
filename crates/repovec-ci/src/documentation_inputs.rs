//! Documentation-input classification and capability-rooted Mermaid detection.

use camino::Utf8Path;
use cap_std::fs_utf8::Dir;

use super::MermaidDetection;

const DOCS_TOOLING_CONFIG_PATHS: &[&str] = &[".markdownlint-cli2.jsonc"];

/// Trims a changed path and removes one leading `./` component.
///
/// For example, `" ./docs/guide.md "` becomes `Some("docs/guide.md")`, while
/// an empty input becomes `None`.
pub(super) fn normalize_path(path: &str) -> Option<String> {
    let trimmed = path.trim();
    if trimmed.is_empty() {
        return None;
    }

    Some(trimmed.strip_prefix("./").map_or_else(|| trimmed.to_owned(), str::to_owned))
}

/// Identifies Markdown files and documentation-tool configuration inputs.
///
/// For example, `"docs/guide.md"` and `".markdownlint-cli2.jsonc"` match, but
/// `"Cargo.toml"` does not.
pub(super) fn is_documentation_input(path: &str) -> bool {
    is_markdown_path(path) || is_docs_tooling_config_path(path)
}

/// Checks a path's extension without regard to letter case.
///
/// For example, `"README.MD"` matches and `"README.txt"` does not.
pub(super) fn is_markdown_path(path: &str) -> bool {
    Utf8Path::new(path).extension().is_some_and(|extension| {
        extension.eq_ignore_ascii_case("md")
            || extension.eq_ignore_ascii_case("markdown")
            || extension.eq_ignore_ascii_case("mdx")
    })
}

/// Checks whether a path is one of the known documentation-tool configs.
///
/// For example, `".markdownlint-cli2.jsonc"` matches; an unrelated JSON file
/// does not.
pub(super) fn is_docs_tooling_config_path(path: &str) -> bool {
    DOCS_TOOLING_CONFIG_PATHS.iter().any(|known_path| path.eq_ignore_ascii_case(known_path))
}

/// Reads a documentation file through the repository-rooted directory handle.
///
/// A Mermaid fence returns `Present`; a readable file without one returns
/// `Absent`; a missing or unreadable path returns `Unknown` so validation can
/// proceed conservatively.
pub(super) fn path_contains_mermaid(root: &Dir, path: &str) -> MermaidDetection {
    root.read_to_string(Utf8Path::new(path)).map_or(MermaidDetection::Unknown, |contents| {
        if contents.contains("```mermaid") {
            MermaidDetection::Present
        } else {
            MermaidDetection::Absent
        }
    })
}
