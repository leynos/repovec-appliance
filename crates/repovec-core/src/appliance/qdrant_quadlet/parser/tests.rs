//! Redaction regression tests for malformed-line telemetry.

use rstest::rstest;

use super::redact_line;

#[rstest]
#[case::quoted_sensitive_assignment_with_spaces(
    r#"Environment=PASSWORD="correct horse battery""#,
    "Environment=<redacted>"
)]
#[case::quoted_nested_sensitive_assignment_with_spaces(
    r#"Environment="QDRANT__SERVICE__API_KEY=correct horse battery""#,
    "Environment=<redacted>"
)]
#[case::quoted_bearer_token_with_spaces(
    r#"Authorization=Bearer "correct horse battery""#,
    "Bearer <redacted>"
)]
#[case::escaped_quotes_inside_sensitive_assignment(
    r#"Environment="DISPLAY_NAME=\"not a boundary\" PASSWORD=secret phrase""#,
    "Environment=<redacted>"
)]
#[case::later_nested_sensitive_assignment(
    r#"Environment="DISPLAY_NAME=public QDRANT__SERVICE__API_KEY=secret phrase""#,
    "Environment=<redacted>"
)]
#[case::quoted_bearer_assignment_value(
    r#"Environment=AUTHORIZATION="Bearer secret phrase""#,
    "Environment=<redacted>"
)]
#[case::nested_bearer_assignment_value(
    r#"Environment="DISPLAY_NAME=public AUTHORIZATION=Bearer secret phrase""#,
    "Environment=<redacted>"
)]
#[case::spaced_nested_sensitive_assignment(
    r#"Environment="QDRANT__SERVICE__API_KEY = correct horse battery""#,
    "Environment=<redacted>"
)]
#[case::hyphenated_api_key_assignment(
    "service-api-key=correct horse battery",
    "service-api-key=<redacted>"
)]
#[case::sensitive_key_over_url_authority_redaction(
    "QDRANT__SERVICE__API_KEY=https://user:pass@example.invalid/path",
    "QDRANT__SERVICE__API_KEY=<redacted>"
)]
fn redact_line_redacts_sensitive_patterns(#[case] line: &str, #[case] expected: &str) {
    assert_eq!(redact_line(line), expected);
}

#[test]
fn redact_line_redacts_unquoted_bearer_token_and_stops() {
    let line = "Authorization: Bearer correct horse battery";
    let redacted_line = redact_line(line);

    assert_eq!(redacted_line, "Authorization: Bearer <redacted>");
    assert!(!redacted_line.contains("correct"));
    assert!(!redacted_line.contains("horse"));
    assert!(!redacted_line.contains("battery"));
}

#[test]
fn redact_line_redacts_unquoted_nested_sensitive_assignment_and_stops() {
    let line = "Environment=QDRANT__SERVICE__API_KEY=correct horse battery";
    let redacted_line = redact_line(line);

    assert_eq!(redacted_line, "Environment=<redacted>");
    assert!(!redacted_line.contains("correct"));
    assert!(!redacted_line.contains("horse"));
    assert!(!redacted_line.contains("battery"));
}

#[test]
fn redact_line_redacts_url_credentials_with_at_in_password() {
    let line = "https://user:p@ss@example.invalid/path";
    let redacted_line = redact_line(line);

    assert_eq!(redacted_line, "https://<redacted>@example.invalid/path");
    assert!(!redacted_line.contains("p@ss"));
}
