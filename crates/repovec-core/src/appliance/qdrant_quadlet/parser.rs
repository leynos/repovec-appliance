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
    let mut tokens = line.split_ascii_whitespace();
    while let Some(token) = tokens.next() {
        if token.eq_ignore_ascii_case("bearer") || token.to_ascii_lowercase().ends_with("=bearer") {
            redacted.push(String::from("Bearer <redacted>"));
            let _ignored_token = tokens.next();
        } else {
            redacted.push(redact_token(token));
        }
    }
    redacted.join(" ")
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
    let Some((nested_key, nested_value)) = unquoted_value.split_once('=') else {
        return false;
    };
    is_sensitive_key(nested_key) || looks_like_secret_value(nested_value)
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
