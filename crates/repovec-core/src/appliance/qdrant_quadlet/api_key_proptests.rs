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
        let expected_payload = api_key.as_ref().map(|(_rendered, _token, payload)| payload.clone());
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
