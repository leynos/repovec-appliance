//! Unit tests for status classification.

use repovec_core::repository_listing::{RateLimitInfo, RepositoryListingError};
use reqwest::{StatusCode, header::HeaderMap};
use rstest::rstest;

use super::classify_error;

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
        classify_error(StatusCode::UNAUTHORIZED, &HeaderMap::new()),
        RepositoryListingError::AuthenticationRejected,
        "401 is an authentication rejection"
    );
}

#[test]
fn a_forbidden_response_without_evidence_is_a_refusal() {
    assert_eq!(
        classify_error(StatusCode::FORBIDDEN, &HeaderMap::new()),
        RepositoryListingError::Forbidden,
        "a bare 403 is a refusal, not a rate limit"
    );
}

#[test]
fn a_forbidden_response_with_retry_after_is_rate_limited() {
    let error = classify_error(StatusCode::FORBIDDEN, &headers(&[("retry-after", "60")]));

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
    let error = classify_error(
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
    let error = classify_error(
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
fn too_many_requests_is_rate_limited_without_metadata() {
    let error = classify_error(StatusCode::TOO_MANY_REQUESTS, &HeaderMap::new());

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
        classify_error(status, &HeaderMap::new()),
        RepositoryListingError::UnexpectedStatus { status: expected },
        "an unmodelled status must surface as unexpected"
    );
}

#[test]
fn a_non_numeric_retry_after_is_ignored() {
    let error = classify_error(
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
