//! Property-based tests for parser tokenisation and redaction invariants.

use proptest::prelude::*;

use super::{line_tokens, redact_line};

fn non_whitespace_content(value: &str) -> String {
    value.chars().filter(|character| !character.is_ascii_whitespace()).collect()
}

prop_compose! {
    fn safe_key_value_pair()(
        key in r"[A-Za-z][A-Za-z0-9_]{0,16}"
            .prop_filter("key must not trigger sensitive redaction", |key| {
                let normalized = key.to_ascii_lowercase();
                ![
                    "api_key",
                    "api-key",
                    "apikey",
                    "authorization",
                    "bearer",
                    "credential",
                    "password",
                    "secret",
                    "token",
                ]
                .iter()
                .any(|needle| normalized.contains(needle))
            }),
        value in r"[A-Za-z0-9]{1,24}",
    ) -> String {
        format!("{key}={value}")
    }
}

proptest! {
    /// Tokenisation removes only inter-token ASCII whitespace.
    #[test]
    fn line_tokens_preserve_non_whitespace_content(line in "\\PC{0,128}") {
        let joined = line_tokens(&line).join(" ");

        prop_assert_eq!(non_whitespace_content(&joined), non_whitespace_content(&line));
    }

    /// Tokenisation never emits empty tokens.
    #[test]
    fn line_tokens_never_emit_empty_tokens(line in "\\PC{0,128}") {
        prop_assert!(line_tokens(&line).iter().all(|token| !token.is_empty()));
    }

    /// Balanced double-quoted regions stay in a single token.
    #[test]
    fn line_tokens_preserve_balanced_double_quoted_region(quoted in r#""[^"\\]*""#) {
        let line = format!("before {quoted} after");

        prop_assert!(line_tokens(&line).iter().any(|token| token == &quoted));
    }

    /// Whitespace inside a quoted value remains part of the token.
    #[test]
    fn line_tokens_preserve_whitespace_in_quoted_assignment(
        value in r"[A-Za-z0-9]+ [A-Za-z0-9]+( [A-Za-z0-9]+){0,3}",
    ) {
        let assignment = format!(r#"key="{value}""#);

        prop_assert_eq!(line_tokens(&assignment), vec![assignment]);
    }

    /// Redacting an already-redacted line is stable.
    #[test]
    fn redact_line_is_idempotent(line in "[^<>]{0,128}") {
        let redacted = redact_line(&line);

        prop_assert_eq!(redact_line(&redacted), redacted);
    }

    /// API-key assignment values never survive redaction.
    #[test]
    fn redact_line_suppresses_sensitive_key_values(secret in "zz[0-9a-f]{8,32}") {
        let line = format!(r#"QDRANT__SERVICE__API_KEY="{secret}""#);

        prop_assert!(!redact_line(&line).contains(&secret));
    }

    /// Bearer token credentials never survive redaction.
    #[test]
    fn redact_line_suppresses_bearer_tokens(token in "zz[0-9a-f]{8,32}") {
        let line = format!("Authorization: Bearer {token}");

        prop_assert!(!redact_line(&line).contains(&token));
    }

    /// Non-sensitive alphanumeric assignments pass through unchanged.
    #[test]
    fn redact_line_preserves_non_sensitive_assignments(
        pairs in prop::collection::vec(safe_key_value_pair(), 1..=5),
    ) {
        let line = pairs.join(" ");

        prop_assert_eq!(redact_line(&line), line);
    }
}
