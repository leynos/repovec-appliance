//! Unit tests for status classification.

use repovec_core::repository_listing::{RateLimitInfo, RepositoryListingError};
use reqwest::{StatusCode, header::HeaderMap};
use rstest::rstest;

use super::classify_error;

/// Classifies a status whose response carried no error body.
fn classify(status: StatusCode, headers: &HeaderMap) -> RepositoryListingError {
    classify_error(status, headers, b"")
}

/// Builds a header map from pairs.
fn headers(pairs: &[(&'static str, &str)]) -> HeaderMap {
    let mut map = HeaderMap::new();
    for (name, text) in pairs {
        let header = reqwest::header::HeaderName::from_static(name);
        let value = reqwest::header::HeaderValue::from_str(text)
            .expect("test header values should be valid");
        map.insert(header, value);
    }
    map
}

#[test]
fn unauthorized_is_an_authentication_rejection() {
    assert_eq!(
        classify(StatusCode::UNAUTHORIZED, &HeaderMap::new()),
        RepositoryListingError::AuthenticationRejected,
        "401 is an authentication rejection"
    );
}

#[test]
fn a_forbidden_response_without_evidence_is_a_refusal() {
    assert_eq!(
        classify(StatusCode::FORBIDDEN, &HeaderMap::new()),
        RepositoryListingError::Forbidden,
        "a bare 403 is a refusal, not a rate limit"
    );
}

#[test]
fn a_forbidden_response_with_retry_after_is_rate_limited() {
    let error = classify(StatusCode::FORBIDDEN, &headers(&[("retry-after", "60")]));

    assert_eq!(
        error,
        RepositoryListingError::RateLimited {
            retry: RateLimitInfo::new(Some(60), None),
        },
        "a 403 carrying retry metadata is a rate limit"
    );
}

#[test]
fn an_exhausted_primary_limit_is_rate_limited() {
    let error = classify(
        StatusCode::FORBIDDEN,
        &headers(&[
            ("x-ratelimit-remaining", "0"),
            ("x-ratelimit-reset", "1700000000"),
        ]),
    );

    assert_eq!(
        error,
        RepositoryListingError::RateLimited {
            retry: RateLimitInfo::new(None, Some(1_700_000_000)),
        },
        "an exhausted primary budget is a rate limit"
    );
}

#[test]
fn a_remaining_budget_without_reset_is_still_a_refusal() {
    let error = classify(
        StatusCode::FORBIDDEN,
        &headers(&[("x-ratelimit-remaining", "42")]),
    );

    assert_eq!(
        error,
        RepositoryListingError::Forbidden,
        "a positive remaining count is not rate-limit evidence"
    );
}

#[test]
fn a_reset_header_with_remaining_quota_is_still_a_refusal() {
    let error = classify(
        StatusCode::FORBIDDEN,
        &headers(&[
            ("x-ratelimit-remaining", "42"),
            ("x-ratelimit-reset", "1700000000"),
        ]),
    );

    assert_eq!(
        error,
        RepositoryListingError::Forbidden,
        "GitHub sends rate-limit headers on ordinary responses, so a reset time alone must not \
         turn an access refusal into a rate limit"
    );
}

#[test]
fn too_many_requests_is_rate_limited_without_metadata() {
    let error = classify(StatusCode::TOO_MANY_REQUESTS, &HeaderMap::new());

    assert_eq!(
        error,
        RepositoryListingError::RateLimited {
            retry: RateLimitInfo::new(None, None),
        },
        "429 is a rate limit even without retry metadata"
    );
}

#[rstest]
#[case::not_modified(StatusCode::NOT_MODIFIED, 304)]
#[case::not_found(StatusCode::NOT_FOUND, 404)]
#[case::unprocessable(StatusCode::UNPROCESSABLE_ENTITY, 422)]
#[case::server_error(StatusCode::INTERNAL_SERVER_ERROR, 500)]
fn other_statuses_are_unexpected(#[case] status: StatusCode, #[case] expected: u16) {
    assert_eq!(
        classify(status, &HeaderMap::new()),
        RepositoryListingError::UnexpectedStatus { status: expected },
        "an unmodelled status must surface as unexpected"
    );
}

#[test]
fn a_non_numeric_retry_after_is_ignored() {
    let error = classify(
        StatusCode::FORBIDDEN,
        &headers(&[("retry-after", "Wed, 21 Oct 2015 07:28:00 GMT")]),
    );

    assert_eq!(
        error,
        RepositoryListingError::RateLimited {
            retry: RateLimitInfo::new(None, None),
        },
        "an HTTP-date retry value is not turned into a bogus number"
    );
}

#[test]
fn a_secondary_limit_message_with_quota_remaining_is_rate_limited() {
    let body = br#"{"message":"You have exceeded a secondary rate limit. Please wait a few minutes before you try again.","documentation_url":"https://docs.github.com/rest"}"#;
    let error = classify_error(
        StatusCode::FORBIDDEN,
        &headers(&[("x-ratelimit-remaining", "4999")]),
        body,
    );

    assert_eq!(
        error,
        RepositoryListingError::RateLimited {
            retry: RateLimitInfo::new(None, None),
        },
        "GitHub documents the error message as the secondary-limit signal, and Retry-After is \
         optional, so the message alone must classify the response"
    );
}

#[test]
fn an_access_message_with_quota_remaining_stays_forbidden() {
    let body = br#"{"message":"Resource not accessible by integration","documentation_url":"https://docs.github.com/rest"}"#;
    let error = classify_error(
        StatusCode::FORBIDDEN,
        &headers(&[("x-ratelimit-remaining", "4999")]),
        body,
    );

    assert_eq!(
        error,
        RepositoryListingError::Forbidden,
        "a message that does not name a rate limit must not become throttling guidance"
    );
}

#[test]
fn an_unparsable_error_body_does_not_change_the_classification() {
    let error = classify_error(
        StatusCode::FORBIDDEN,
        &headers(&[("x-ratelimit-remaining", "4999")]),
        b"<html><body>Forbidden</body></html>",
    );

    assert_eq!(
        error,
        RepositoryListingError::Forbidden,
        "a proxy error page is not evidence of a rate limit"
    );
}

#[test]
fn a_rate_limit_message_on_a_429_reports_the_validated_reset() {
    let body = br#"{"message":"API rate limit exceeded for user ID 1."}"#;
    let error = classify_error(
        StatusCode::TOO_MANY_REQUESTS,
        &headers(&[("x-ratelimit-reset", "1700000000")]),
        body,
    );

    assert_eq!(
        error,
        RepositoryListingError::RateLimited {
            retry: RateLimitInfo::new(None, Some(1_700_000_000)),
        },
        "the message must not displace the reset metadata the headers carry"
    );
}
