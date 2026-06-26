//! Unit tests for API-key environment assignment tokenisation.

use proptest::prelude::*;
use rstest::rstest;

use super::{
    QDRANT_API_KEY_ENVIRONMENT_VARIABLE,
    api_key::{
        is_api_key_environment_assignment, redact_api_key_environment_assignment,
        split_environment_assignments,
    },
};

#[derive(Clone, Copy, Debug)]
enum QuoteStyle {
    Bare,
    Single,
    Double,
}

fn quote_style_strategy() -> impl Strategy<Value = QuoteStyle> {
    prop_oneof![Just(QuoteStyle::Bare), Just(QuoteStyle::Single), Just(QuoteStyle::Double)]
}

fn quoted_assignment(key: &str, value: &str, quote_style: QuoteStyle) -> String {
    match quote_style {
        QuoteStyle::Bare => format!("{key}={}", value.replace(' ', "_")),
        QuoteStyle::Single => format!("{key}='{value}'"),
        QuoteStyle::Double => format!(r#"{key}="{value}""#),
    }
}

prop_compose! {
    fn filler_assignment_strategy()(
        key_suffix in "[A-Z0-9_]{0,8}",
        value in "[A-Za-z0-9_ -]{0,24}",
        quote_style in quote_style_strategy(),
    ) -> String {
        quoted_assignment(&format!("AUX{key_suffix}"), &value, quote_style)
    }
}

prop_compose! {
    fn api_key_assignment_strategy()(
        value in "[A-Za-z0-9_ -]{1,24}",
        quote_style in quote_style_strategy(),
    ) -> (String, String) {
        let rendered_assignment =
            quoted_assignment(QDRANT_API_KEY_ENVIRONMENT_VARIABLE, &value, quote_style);
        let expected_value = match quote_style {
            QuoteStyle::Bare => value.replace(' ', "_"),
            QuoteStyle::Single | QuoteStyle::Double => value,
        };
        (
            rendered_assignment,
            format!("{QDRANT_API_KEY_ENVIRONMENT_VARIABLE}={expected_value}"),
        )
    }
}

#[rstest]
#[case::double_quoted_values_with_spaces(
    r#"FOO="hello world" BAR=baz"#,
    vec!["FOO=hello world", "BAR=baz"],
)]
#[case::single_quoted_values_with_spaces(
    "FOO='hello world' BAR=baz",
    vec!["FOO=hello world", "BAR=baz"],
)]
#[case::repeated_whitespace(
    "  FOO=bar \t  BAR=baz  ",
    vec!["FOO=bar", "BAR=baz"],
)]
#[case::unmatched_quote(
    r#"FOO="unterminated value BAR=baz"#,
    vec!["FOO=unterminated value BAR=baz"],
)]
#[case::escaped_quotes_inside_quoted_value(
    r#"FOO="hello \"quoted\" world" BAR=baz"#,
    vec![r#"FOO=hello \"quoted\" world"#, "BAR=baz"],
)]
#[case::apostrophe_inside_unquoted_value_does_not_merge_assignments(
    "AUTHOR=O'Reilly QDRANT__SERVICE__API_KEY=secret",
    vec!["AUTHOR=O'Reilly", "QDRANT__SERVICE__API_KEY=secret"],
)]
fn split_environment_assignments_preserves_quote_aware_assignments(
    #[case] environment: &str,
    #[case] expected: Vec<&str>,
) {
    assert_eq!(split_environment_assignments(environment), expected);
}

proptest! {
    #[test]
    fn api_key_environment_assignment_is_split_detected_and_redacted(
        before in proptest::collection::vec(filler_assignment_strategy(), 0..3),
        api_key_assignment in api_key_assignment_strategy(),
        after in proptest::collection::vec(filler_assignment_strategy(), 0..3),
        separator in "[ \t]{1,4}",
    ) {
        let (rendered_api_key_assignment, expected_api_key_assignment) = api_key_assignment;
        let mut rendered_assignments = before;
        rendered_assignments.push(rendered_api_key_assignment);
        rendered_assignments.extend(after);
        let environment = rendered_assignments.join(&separator);

        let assignments = split_environment_assignments(&environment);

        prop_assert!(assignments.contains(&expected_api_key_assignment));
        prop_assert!(is_api_key_environment_assignment(&expected_api_key_assignment));
        prop_assert_eq!(
            redact_api_key_environment_assignment(&expected_api_key_assignment),
            format!("{QDRANT_API_KEY_ENVIRONMENT_VARIABLE}=<redacted>"),
        );
    }
}

#[test]
fn is_api_key_environment_assignment_detects_bare_variable() {
    assert!(is_api_key_environment_assignment(QDRANT_API_KEY_ENVIRONMENT_VARIABLE));
}

#[test]
fn redact_api_key_environment_assignment_formats_key_value_pair() {
    assert_eq!(
        redact_api_key_environment_assignment("QDRANT__SERVICE__API_KEY=secret"),
        "QDRANT__SERVICE__API_KEY=<redacted>",
    );
}
