//! Quote-aware tokenization of Quadlet `Environment=` assignments.
//!
//! Quadlet `Environment=` values may carry several `KEY=VALUE` pairs separated
//! by whitespace, and a value may itself contain whitespace when it is quoted.
//! Splitting on whitespace alone would therefore tear a quoted value in half and
//! mis-attribute the remainder to a neighbouring key, which matters here because
//! the API-key prohibition is a substring-free exact match on the key.
//!
//! These helpers are kept away from the validators that call them so the
//! tokenizer can be reviewed, and property-tested, as a self-contained parse.

use super::QDRANT_API_KEY_ENVIRONMENT_VARIABLE;

/// Replaces the value of an API-key assignment with a redaction marker.
///
/// Diagnostics must never echo the API key itself, so the returned string keeps
/// the key name and discards everything after the first `=`.
pub(super) fn redact_api_key_environment_assignment(assignment: &str) -> String {
    match assignment.split_once('=') {
        Some((key, _)) => format!("{key}=<redacted>"),
        None => assignment.to_owned(),
    }
}

/// Splits an `Environment=` value into individual `KEY=VALUE` assignments.
///
/// The scan is quote-aware because Quadlet `Environment=` values may contain
/// spaces inside quoted `KEY=VALUE` pairs. The quote state records when
/// whitespace belongs to the current assignment rather than separating
/// assignments.
///
/// Escaped characters inside quoted strings are kept in the current assignment.
/// Unmatched quotes are treated as part of the current assignment until the scan
/// ends. Empty assignments created by repeated whitespace are skipped.
pub(super) fn split_environment_assignments(environment: &str) -> Vec<String> {
    let mut assignments = Vec::new();
    let mut assignment = String::new();
    let mut quote = None;
    let mut is_escaped = false;

    for character in environment.chars() {
        if is_escaped {
            assignment.push(character);
            is_escaped = false;
            continue;
        }

        match (quote, character) {
            (Some(_), '\\') => {
                assignment.push(character);
                is_escaped = true;
            }
            (Some(active_quote), current) if active_quote == current => quote = None,
            (None, '"' | '\'') if assignment.is_empty() || assignment.ends_with('=') => {
                quote = Some(character);
            }
            (None, current) if current.is_ascii_whitespace() => {
                if !assignment.is_empty() {
                    assignments.push(std::mem::take(&mut assignment));
                }
            }
            _ => assignment.push(character),
        }
    }

    if !assignment.is_empty() {
        assignments.push(assignment);
    }

    assignments
}

/// Reports whether an assignment sets the Qdrant API-key environment variable.
///
/// A bare variable name counts as an assignment too: Quadlet would inherit the
/// value from the manager's environment, which is just as much an inline key as
/// an explicit `KEY=VALUE` pair.
pub(super) fn is_api_key_environment_assignment(assignment: &str) -> bool {
    assignment == QDRANT_API_KEY_ENVIRONMENT_VARIABLE
        || assignment
            .split_once('=')
            .is_some_and(|(key, _value)| key == QDRANT_API_KEY_ENVIRONMENT_VARIABLE)
}

#[cfg(test)]
mod proptests {
    //! Property tests for API-key environment assignment parsing.

    use proptest::prelude::*;

    use super::{
        QDRANT_API_KEY_ENVIRONMENT_VARIABLE, is_api_key_environment_assignment,
        redact_api_key_environment_assignment, split_environment_assignments,
    };

    const REDACTED_API_KEY: &str = "QDRANT__SERVICE__API_KEY=<redacted>";

    #[derive(Clone, Copy, Debug)]
    enum QuoteStyle {
        Bare,
        Single,
        Double,
    }

    fn quote_style_strategy() -> impl Strategy<Value = QuoteStyle> {
        prop_oneof![Just(QuoteStyle::Bare), Just(QuoteStyle::Single), Just(QuoteStyle::Double)]
    }

    fn assignment(key: &str, value: &str, quote_style: QuoteStyle) -> (String, String) {
        match quote_style {
            QuoteStyle::Bare => {
                let bare_value = value.replace(' ', "_");
                (format!("{key}={bare_value}"), format!("{key}={bare_value}"))
            }
            QuoteStyle::Single => (format!("{key}='{value}'"), format!("{key}={value}")),
            QuoteStyle::Double => (format!(r#"{key}="{value}""#), format!("{key}={value}")),
        }
    }

    prop_compose! {
        fn neighbouring_assignment()(
            key in "[A-Z][A-Z0-9_]{0,8}",
            value in "zz[0-9A-F _-]{1,18}",
            quote_style in quote_style_strategy(),
        ) -> String {
            assignment(&key, &value, quote_style).0
        }
    }

    prop_compose! {
        fn api_key_assignment()(
            value in "zz[0-9A-F _-]{1,18}",
            quote_style in quote_style_strategy(),
        ) -> (String, String, String) {
            let (rendered, token) =
                assignment(QDRANT_API_KEY_ENVIRONMENT_VARIABLE, &value, quote_style);
            let payload = token
                .split_once('=')
                .map_or_else(String::new, |(_key, assignment_value)| {
                    assignment_value.to_owned()
                });
            (rendered, token, payload)
        }
    }

    proptest! {
        #[test]
        fn api_key_assignment_survives_splitting(
            before in proptest::collection::vec(neighbouring_assignment(), 1..4),
            api_key in api_key_assignment(),
            mut after in proptest::collection::vec(neighbouring_assignment(), 0..3),
            separator in "[ \t]{1,4}",
        ) {
            after.push(String::from("BROKEN='unterminated value"));
            let (rendered_api_key, expected_api_key, _payload) = api_key;
            let mut rendered = before;
            rendered.push(rendered_api_key);
            rendered.extend(after);

            let tokens = split_environment_assignments(&rendered.join(&separator));

            prop_assert!(tokens.contains(&expected_api_key));
            let api_key_is_isolated = tokens.iter().all(|token| {
                token == &expected_api_key || !token.contains(QDRANT_API_KEY_ENVIRONMENT_VARIABLE)
            });
            prop_assert!(api_key_is_isolated);
        }

        #[test]
        fn api_key_assignment_is_detected(value in "zz[0-9A-F_./:-]{1,32}") {
            let assignment = format!("{QDRANT_API_KEY_ENVIRONMENT_VARIABLE}={value}");

            prop_assert!(is_api_key_environment_assignment(&assignment));
            prop_assert!(is_api_key_environment_assignment(QDRANT_API_KEY_ENVIRONMENT_VARIABLE));
        }

        #[test]
        fn api_key_assignment_is_redacted(value in "zz[0-9A-F _-]{1,32}") {
            let assignment = format!("{QDRANT_API_KEY_ENVIRONMENT_VARIABLE}={value}");
            let redacted = redact_api_key_environment_assignment(&assignment);

            prop_assert_eq!(redacted.as_str(), REDACTED_API_KEY);
            prop_assert!(!redacted.contains(&value));
        }

        #[test]
        fn split_detect_redact_pipeline_holds(
            before in proptest::collection::vec(neighbouring_assignment(), 0..3),
            api_key in prop_oneof![
                api_key_assignment().prop_map(Some),
                Just(None),
            ],
            after in proptest::collection::vec(neighbouring_assignment(), 0..3),
            separator in "[ \t]{1,4}",
        ) {
            let mut rendered_assignments = before;
            let expected_payload = api_key
                .as_ref()
                .map(|(_rendered, _token, payload)| payload.clone());
            rendered_assignments.push(api_key.map_or_else(
                || QDRANT_API_KEY_ENVIRONMENT_VARIABLE.to_owned(),
                |(rendered_assignment, _token, _payload)| rendered_assignment,
            ));
            rendered_assignments.extend(after);

            for token in split_environment_assignments(&rendered_assignments.join(&separator)) {
                if is_api_key_environment_assignment(&token) {
                    let redacted = redact_api_key_environment_assignment(&token);
                    if token == QDRANT_API_KEY_ENVIRONMENT_VARIABLE {
                        prop_assert_eq!(redacted, QDRANT_API_KEY_ENVIRONMENT_VARIABLE);
                    } else {
                        prop_assert_eq!(redacted.as_str(), REDACTED_API_KEY);
                        if let Some(payload) = &expected_payload {
                            prop_assert!(!redacted.contains(payload));
                        }
                    }
                }
            }
        }
    }
}
