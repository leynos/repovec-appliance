//! Documentation-input classification and capability-rooted Mermaid detection.

use camino::Utf8Path;
use cap_std::fs_utf8::Dir;

use super::MermaidDetection;

const DOCS_TOOLING_CONFIG_PATHS: &[&str] = &[".markdownlint-cli2.jsonc"];

pub(super) fn normalize_path(path: &str) -> Option<String> {
    let trimmed = path.trim();
    if trimmed.is_empty() {
        return None;
    }

    Some(trimmed.strip_prefix("./").map_or_else(|| trimmed.to_owned(), str::to_owned))
}

pub(super) fn is_documentation_input(path: &str) -> bool {
    is_markdown_path(path) || is_docs_tooling_config_path(path)
}

pub(super) fn is_markdown_path(path: &str) -> bool {
    Utf8Path::new(path).extension().is_some_and(|extension| {
        extension.eq_ignore_ascii_case("md")
            || extension.eq_ignore_ascii_case("markdown")
            || extension.eq_ignore_ascii_case("mdx")
    })
}

pub(super) fn is_docs_tooling_config_path(path: &str) -> bool {
    DOCS_TOOLING_CONFIG_PATHS.iter().any(|known_path| path.eq_ignore_ascii_case(known_path))
}

pub(super) fn path_contains_mermaid(root: &Dir, path: &str) -> MermaidDetection {
    root.read_to_string(Utf8Path::new(path)).map_or(MermaidDetection::Unknown, |contents| {
        if contents.contains("```mermaid") {
            MermaidDetection::Present
        } else {
            MermaidDetection::Absent
        }
    })
}
