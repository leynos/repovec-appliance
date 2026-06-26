//! Fixed-case tests for API-key environment assignment tokenisation.

use rstest::rstest;

use super::{
    QDRANT_API_KEY_ENVIRONMENT_VARIABLE,
    api_key::{
        is_api_key_environment_assignment, redact_api_key_environment_assignment,
        split_environment_assignments,
    },
};

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
