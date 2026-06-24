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

#[cfg(test)]
mod proptests;
#[cfg(test)]
mod tests;

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
            break;
        }

        let redacted_token = redact_token(token.as_str());
        let should_stop = redacted_token.is_sensitive_assignment;
        redacted.push(redacted_token.value);
        if should_stop {
            break;
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

struct RedactedToken {
    value: String,
    is_sensitive_assignment: bool,
}

fn redact_token(token: &str) -> RedactedToken {
    if let Some((key, value)) = token.split_once('=') {
        return if is_sensitive_assignment(key, value) {
            RedactedToken { value: format!("{key}=<redacted>"), is_sensitive_assignment: true }
        } else {
            RedactedToken { value: redact_url_authority(token), is_sensitive_assignment: false }
        };
    }

    RedactedToken { value: redact_url_authority(token), is_sensitive_assignment: false }
}

fn is_sensitive_assignment(key: &str, value: &str) -> bool {
    if is_sensitive_key(key) || looks_like_secret_value(value) {
        return true;
    }

    let unquoted_value = trim_surrounding_quotes(value);
    if looks_like_secret_value(unquoted_value) {
        return true;
    }

    let tokens = line_tokens(unquoted_value);
    tokens.iter().enumerate().any(|(index, raw_token)| {
        let trimmed_token = raw_token.trim();
        if trimmed_token.eq_ignore_ascii_case("bearer")
            && tokens.get(index + 1).is_some_and(|next| !next.trim().is_empty())
        {
            return true;
        }
        if is_spaced_sensitive_assignment(&tokens, index, trimmed_token) {
            return true;
        }

        trimmed_token.split_once('=').is_some_and(|(raw_nested_key, raw_nested_value)| {
            let trimmed_nested_key = raw_nested_key.trim();
            let trimmed_nested_value = raw_nested_value.trim();
            is_sensitive_key(trimmed_nested_key)
                || looks_like_secret_value(trimmed_nested_value)
                || (trimmed_nested_value.eq_ignore_ascii_case("bearer")
                    && tokens.get(index + 1).is_some_and(|next| !next.trim().is_empty()))
        })
    })
}

fn is_spaced_sensitive_assignment(tokens: &[String], index: usize, key: &str) -> bool {
    if !is_sensitive_key(key) {
        return false;
    }

    let has_separator = tokens.get(index + 1).is_some_and(|next| next.trim() == "=");
    let has_value = tokens.get(index + 2).is_some_and(|next| !next.trim().is_empty());
    has_separator && has_value
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
    let Some((_credentials, host_and_path)) = rest.rsplit_once('@') else {
        return token.to_owned();
    };
    format!("{scheme}://<redacted>@{host_and_path}")
}

fn is_sensitive_key(key: &str) -> bool {
    let normalized = key.to_ascii_lowercase();
    normalized.contains("api_key")
        || normalized.contains("api-key")
        || normalized.contains("apikey")
        || normalized.contains("authorization")
        || normalized.contains("bearer")
        || normalized.contains("credential")
        || normalized.contains("password")
        || normalized.contains("secret")
        || normalized.contains("token")
}

fn looks_like_secret_value(value: &str) -> bool {
    let unquoted_value = trim_surrounding_quotes(value).trim();
    let mut parts = unquoted_value.splitn(2, char::is_whitespace);
    let Some(scheme) = parts.next() else {
        return false;
    };
    scheme.eq_ignore_ascii_case("bearer")
        && parts.next().is_some_and(|token| !token.trim().is_empty())
}
