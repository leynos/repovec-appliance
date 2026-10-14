//! Redaction helpers for token-store diagnostics.

use std::fmt;

/// Lossy standard-error text that avoids panics on invalid UTF-8.
#[derive(Clone, Eq, PartialEq)]
pub struct LossyStderr(pub(super) String);

impl fmt::Debug for LossyStderr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("LossyStderr").field(&redact_tokenish_words(&self.0)).finish()
    }
}

impl fmt::Display for LossyStderr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&redact_tokenish_words(&self.0))
    }
}

/// Replaces whitespace-delimited words containing a GitHub token prefix.
/// For example, `failed <token-bearing-word>` becomes `failed [redacted]`.
fn redact_tokenish_words(input: &str) -> String {
    input
        .split_whitespace()
        .map(|word| if contains_github_token_prefix(word) { "[redacted]" } else { word })
        .collect::<Vec<_>>()
        .join(" ")
}

/// Checks whether a word contains any recognized GitHub token prefix.
/// For example, a word containing `gho_` is recognized even after punctuation.
fn contains_github_token_prefix(word: &str) -> bool {
    ["gho_", "ghp_", "ghs_", "ghr_", "ghu_", "github_pat_"]
        .iter()
        .any(|prefix| word.contains(prefix))
}
