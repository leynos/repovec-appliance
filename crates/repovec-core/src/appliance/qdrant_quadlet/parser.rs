//! Internal section-aware parser for the parent `qdrant_quadlet` module.
//!
//! This is a purpose-built INI-style parser for the appliance's Qdrant Quadlet
//! subset. It is not a general-purpose Quadlet or systemd parser.
//!
//! The module-family surface is [`ParsedQuadlet`] and
//! [`ParsedQuadlet::parse`]. `ParsedQuadlet` stores section data as section to
//! key to `Vec<String>`, preserving repeated directives so the parent
//! `qdrant_quadlet` validators can inspect multi-value entries such as
//! `Requires=`, `Secret=`, and `PublishPort=`.
//!
//! Parsing rejects malformed input with [`QdrantQuadletError::InvalidLine`]
//! when a non-comment, non-header line lacks `=`, and with
//! [`QdrantQuadletError::PropertyBeforeSection`] when a key/value pair appears
//! before any section header. Those cases are reported through the injected
//! observer with redacted line content, while the returned error still carries
//! the trimmed original line.
//!
//! The parser is private to the `qdrant_quadlet` module family and feeds the
//! structural validators in the parent module.

use std::collections::BTreeMap;

use super::{QdrantQuadletError, observer::QdrantQuadletObserver};

#[derive(Debug)]
pub(super) struct ParsedQuadlet {
    sections: BTreeMap<String, BTreeMap<String, Vec<String>>>,
}

impl ParsedQuadlet {
    pub(super) fn parse(
        contents: &str,
        observer: &dyn QdrantQuadletObserver,
    ) -> Result<Self, QdrantQuadletError> {
        let mut sections = BTreeMap::<String, BTreeMap<String, Vec<String>>>::new();
        let mut current_section: Option<String> = None;

        for (line_index, raw_line) in contents.lines().enumerate() {
            let line_number = line_index + 1;
            let line = raw_line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }

            if let Some(section) = parse_section_header(line) {
                current_section = Some(section.to_owned());
                sections.entry(section.to_owned()).or_default();
                continue;
            }

            let Some((key, value)) = line.split_once('=') else {
                let redacted_line = redact_line(line);
                observer.invalid_line(line_number, &redacted_line);
                return Err(QdrantQuadletError::InvalidLine { line_number, line: line.to_owned() });
            };

            let Some(section) = &current_section else {
                let redacted_line = redact_line(line);
                observer.property_before_section(line_number, &redacted_line);
                return Err(QdrantQuadletError::PropertyBeforeSection {
                    line_number,
                    line: line.to_owned(),
                });
            };

            sections
                .entry(section.clone())
                .or_default()
                .entry(key.trim().to_owned())
                .or_default()
                .push(value.trim().to_owned());
        }

        Ok(Self { sections })
    }

    pub(super) fn values(&self, section: &str, key: &str) -> &[String] {
        self.sections.get(section).and_then(|entries| entries.get(key)).map_or(&[], Vec::as_slice)
    }
}

fn parse_section_header(line: &str) -> Option<&str> { line.strip_prefix('[')?.strip_suffix(']') }

fn redact_line(line: &str) -> String {
    let mut redacted = Vec::new();
    let mut tokens = line_tokens(line).into_iter();
    while let Some(token) = tokens.next() {
        if token.eq_ignore_ascii_case("bearer") || token.to_ascii_lowercase().ends_with("=bearer") {
            redacted.push(String::from("Bearer <redacted>"));
            let _ignored_token = tokens.next();
        } else {
            redacted.push(redact_token(token.as_str()));
        }
    }
    redacted.join(" ")
}

fn line_tokens(line: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let mut token = String::new();
    let mut active_quote = None;
    let mut is_escaped = false;

    for character in line.chars() {
        if is_escaped {
            token.push(character);
            is_escaped = false;
            continue;
        }

        match (character, active_quote) {
            ('\\', _) => {
                token.push(character);
                is_escaped = true;
            }
            (quote @ ('\'' | '"'), None) => {
                active_quote = Some(quote);
                token.push(character);
            }
            (quote, Some(active)) if quote == active => {
                active_quote = None;
                token.push(character);
            }
            (whitespace, None) if whitespace.is_ascii_whitespace() => {
                if !token.is_empty() {
                    tokens.push(std::mem::take(&mut token));
                }
            }
            _ => {
                token.push(character);
            }
        }
    }

    if !token.is_empty() {
        tokens.push(token);
    }

    tokens
}

fn redact_token(token: &str) -> String {
    if token.contains("://") && token.contains('@') {
        return redact_url_authority(token);
    }

    let Some((key, value)) = token.split_once('=') else { return token.to_owned() };

    if is_sensitive_assignment(key, value) { format!("{key}=<redacted>") } else { token.to_owned() }
}

fn is_sensitive_assignment(key: &str, value: &str) -> bool {
    if is_sensitive_key(key) || looks_like_secret_value(value) {
        return true;
    }

    let unquoted_value = trim_surrounding_quotes(value);
    line_tokens(unquoted_value).into_iter().any(|token| {
        token.split_once('=').is_some_and(|(nested_key, nested_value)| {
            is_sensitive_key(nested_key) || looks_like_secret_value(nested_value)
        })
    })
}

fn trim_surrounding_quotes(value: &str) -> &str {
    value
        .strip_prefix('"')
        .and_then(|unquoted| unquoted.strip_suffix('"'))
        .or_else(|| value.strip_prefix('\'').and_then(|unquoted| unquoted.strip_suffix('\'')))
        .unwrap_or(value)
}

fn redact_url_authority(token: &str) -> String {
    let Some((scheme, rest)) = token.split_once("://") else {
        return token.to_owned();
    };
    let Some((_credentials, host_and_path)) = rest.split_once('@') else {
        return token.to_owned();
    };
    format!("{scheme}://<redacted>@{host_and_path}")
}

fn is_sensitive_key(key: &str) -> bool {
    let normalized = key.to_ascii_lowercase();
    normalized.contains("api_key")
        || normalized.contains("apikey")
        || normalized.contains("authorization")
        || normalized.contains("bearer")
        || normalized.contains("credential")
        || normalized.contains("password")
        || normalized.contains("secret")
        || normalized.contains("token")
}

fn looks_like_secret_value(value: &str) -> bool {
    value
        .strip_prefix("Bearer ")
        .or_else(|| value.strip_prefix("bearer "))
        .is_some_and(|token| !token.trim().is_empty())
}

#[cfg(test)]
mod tests {
    //! Redaction regression tests for malformed-line telemetry.

    use super::redact_line;

    #[test]
    fn redact_line_redacts_quoted_sensitive_assignment_with_spaces() {
        let line = r#"Environment=PASSWORD="correct horse battery""#;

        assert_eq!(redact_line(line), "Environment=<redacted>");
    }

    #[test]
    fn redact_line_redacts_quoted_nested_sensitive_assignment_with_spaces() {
        let line = r#"Environment="QDRANT__SERVICE__API_KEY=correct horse battery""#;

        assert_eq!(redact_line(line), "Environment=<redacted>");
    }

    #[test]
    fn redact_line_redacts_quoted_bearer_token_with_spaces() {
        let line = r#"Authorization=Bearer "correct horse battery""#;

        assert_eq!(redact_line(line), "Bearer <redacted>");
    }

    #[test]
    fn redact_line_keeps_escaped_quotes_inside_sensitive_assignment() {
        let line = r#"Environment="DISPLAY_NAME=\"not a boundary\" PASSWORD=secret phrase""#;

        assert_eq!(redact_line(line), "Environment=<redacted>");
    }

    #[test]
    fn redact_line_redacts_later_nested_sensitive_assignment() {
        let line = r#"Environment="DISPLAY_NAME=public QDRANT__SERVICE__API_KEY=secret phrase""#;

        assert_eq!(redact_line(line), "Environment=<redacted>");
    }
}

#[cfg(test)]
mod proptests {
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
}
