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

#[test]
fn redact_line_redacts_quoted_bearer_assignment_value() {
    let line = r#"Environment=AUTHORIZATION="Bearer secret phrase""#;

    assert_eq!(redact_line(line), "Environment=<redacted>");
}

#[test]
fn redact_line_redacts_nested_bearer_assignment_value() {
    let line = r#"Environment="DISPLAY_NAME=public AUTHORIZATION=Bearer secret phrase""#;

    assert_eq!(redact_line(line), "Environment=<redacted>");
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
fn redact_line_redacts_spaced_nested_sensitive_assignment() {
    let line = r#"Environment="QDRANT__SERVICE__API_KEY = correct horse battery""#;

    assert_eq!(redact_line(line), "Environment=<redacted>");
}

#[test]
fn redact_line_redacts_hyphenated_api_key_assignment() {
    let line = "service-api-key=correct horse battery";

    assert_eq!(redact_line(line), "service-api-key=<redacted>");
}

#[test]
fn redact_line_redacts_url_credentials_with_at_in_password() {
    let line = "https://user:p@ss@example.invalid/path";
    let redacted_line = redact_line(line);

    assert_eq!(redacted_line, "https://<redacted>@example.invalid/path");
    assert!(!redacted_line.contains("p@ss"));
}

#[test]
fn redact_line_prefers_sensitive_key_over_url_authority_redaction() {
    let line = "QDRANT__SERVICE__API_KEY=https://user:pass@example.invalid/path";

    assert_eq!(redact_line(line), "QDRANT__SERVICE__API_KEY=<redacted>");
}
