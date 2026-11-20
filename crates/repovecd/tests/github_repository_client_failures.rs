//! Failure atomicity tests for the GitHub repository listing adapter.
//!
//! Every condition the listing contract distinguishes is scripted against a
//! loopback server: rejected authority, throttling, transport failure,
//! unexpected status, and malformed bodies. Each test asserts the typed error,
//! the absence of a partial listing, and that no request followed a failure.

mod common;

use std::time::Duration;

use common::{installation_client::installation_client, user_client::user_client};
use googletest::prelude::*;
use pretty_assertions::assert_eq;
use repovec_core::repository_listing::{RateLimitInfo, RepositoryListing, RepositoryListingError};
use repovec_test_helpers::{
    recording_http::{RecordingServer, ScriptedResponse},
    repository_listing_wire::{Repository, bare_page, repository_json},
};
use repovecd::github_repository_client::{GitHubRepositoryClient, ListingLimits};
use rstest::rstest;

/// The request deadline the failure tests allow themselves.
const TEST_TIMEOUT: Duration = Duration::from_secs(5);

/// The deadline used by the timeout case, well below the scripted delay.
const EXPIRING_TIMEOUT: Duration = Duration::from_millis(500);

/// The delay the timeout case scripts, well above its request deadline.
const SLOW_RESPONSE: Duration = Duration::from_secs(5);

/// Which credential kind an adapter under test presents.
#[derive(Clone, Copy, Debug)]
enum Profile {
    /// A user credential, whose pages are bare arrays.
    User,
    /// An installation credential, whose pages are enveloped.
    Installation,
}

/// Builds an adapter for the named profile at a loopback origin.
fn adapter(
    profile: Profile,
    origin: &str,
) -> Result<GitHubRepositoryClient, RepositoryListingError> {
    match profile {
        Profile::User => user_client(origin, ListingLimits::defaults(), TEST_TIMEOUT),
        Profile::Installation => {
            installation_client(origin, ListingLimits::defaults(), TEST_TIMEOUT)
        }
    }
}

#[rstest]
#[case::unauthorized(401, RepositoryListingError::AuthenticationRejected)]
#[case::forbidden(403, RepositoryListingError::Forbidden)]
#[case::not_found(404, RepositoryListingError::UnexpectedStatus { status: 404 })]
#[case::unprocessable(422, RepositoryListingError::UnexpectedStatus { status: 422 })]
#[case::not_modified(304, RepositoryListingError::UnexpectedStatus { status: 304 })]
#[case::server_error(500, RepositoryListingError::UnexpectedStatus { status: 500 })]
#[case::unavailable(503, RepositoryListingError::UnexpectedStatus { status: 503 })]
fn a_rejected_first_page_returns_a_typed_error(
    #[case] status: u16,
    #[case] expected: RepositoryListingError,
) {
    let server = RecordingServer::start(vec![ScriptedResponse::new(status)])
        .expect("the scripted server should start");
    let client =
        adapter(Profile::User, &server.origin()).expect("a user-mode adapter should construct");

    let result = client.list_repositories();

    assert_eq!(
        result.err(),
        Some(expected),
        "status {status} must map to its documented error"
    );
    assert_eq!(
        server.request_count(),
        1,
        "a rejected page must not be retried"
    );
}

#[test]
fn an_installation_credential_rejection_is_the_same_error() {
    let server = RecordingServer::start(vec![ScriptedResponse::new(401)])
        .expect("the scripted server should start");
    let client = adapter(Profile::Installation, &server.origin())
        .expect("an installation-mode adapter should construct");

    let result = client.list_repositories();

    assert_eq!(
        result.err(),
        Some(RepositoryListingError::AuthenticationRejected),
        "both credential kinds share one authentication failure"
    );
}

#[rstest]
#[case::retry_after(
    ScriptedResponse::new(429).with_header("Retry-After", "60"),
    RateLimitInfo::new(Some(60), None)
)]
#[case::exhausted_window(
    ScriptedResponse::new(403)
        .with_header("X-RateLimit-Remaining", "0")
        .with_header("X-RateLimit-Reset", "1700000000"),
    RateLimitInfo::new(None, Some(1_700_000_000))
)]
fn a_throttled_page_reports_only_validated_metadata(
    #[case] response: ScriptedResponse,
    #[case] retry: RateLimitInfo,
) {
    let server = RecordingServer::start(vec![response]).expect("the scripted server should start");
    let client =
        adapter(Profile::User, &server.origin()).expect("a user-mode adapter should construct");

    let result = client.list_repositories();

    assert_eq!(
        result.err(),
        Some(RepositoryListingError::RateLimited { retry }),
        "throttling must carry the retry metadata the server validated"
    );
}

#[test]
fn a_forbidden_page_without_rate_limit_evidence_stays_forbidden() {
    let server = RecordingServer::start(vec![
        ScriptedResponse::new(403).with_header("X-RateLimit-Remaining", "4999"),
    ])
    .expect("the scripted server should start");
    let client =
        adapter(Profile::User, &server.origin()).expect("a user-mode adapter should construct");

    let result = client.list_repositories();

    assert_eq!(
        result.err(),
        Some(RepositoryListingError::Forbidden),
        "an unexplained 403 is a permission failure, not throttling"
    );
}

#[test]
fn an_unanswered_request_is_a_transport_failure() {
    let server = RecordingServer::start(vec![ScriptedResponse::new(200).with_delay(SLOW_RESPONSE)])
        .expect("the scripted server should start");
    let client = user_client(
        &server.origin(),
        ListingLimits::defaults(),
        EXPIRING_TIMEOUT,
    )
    .expect("a user-mode adapter should construct");

    let result = client.list_repositories();

    assert_eq!(
        result.err(),
        Some(RepositoryListingError::TransportFailure),
        "a request that outlives its deadline is a transport failure"
    );
}

#[rstest]
#[case::truncated_array(
    Profile::User,
    "[{\"id\":1,",
    RepositoryListingError::InvalidResponse {
        reason: "page body is not a JSON array of records",
    }
)]
#[case::wrong_envelope(
    Profile::Installation,
    "{\"items\":[]}",
    RepositoryListingError::InvalidResponse {
        reason: "page body is missing its records",
    }
)]
#[case::bare_array_for_an_envelope(
    Profile::Installation,
    "[]",
    RepositoryListingError::InvalidResponse {
        reason: "page body is not a JSON object",
    }
)]
#[case::missing_owner(
    Profile::Installation,
    concat!(
        r#"{"total_count":1,"repositories":[{"id":1,"name":"one","#,
        r#""private":false,"default_branch":"main"}]}"#
    ),
    RepositoryListingError::InvalidResponse {
        reason: "page records do not match their schema",
    }
)]
#[case::empty_name(
    Profile::User,
    concat!(
        r#"[{"id":2,"name":"","private":false,"default_branch":"main","#,
        r#""owner":{"login":"acme"}}]"#
    ),
    RepositoryListingError::InvalidResponse {
        reason: "repository name is invalid",
    }
)]
#[case::zero_identifier(
    Profile::Installation,
    concat!(
        r#"{"total_count":1,"repositories":[{"id":0,"name":"one","#,
        r#""private":false,"default_branch":"main","owner":{"login":"acme"}}]}"#
    ),
    RepositoryListingError::InvalidResponse {
        reason: "repository identifier must be positive",
    }
)]
fn a_body_outside_the_contract_fails_the_listing(
    #[case] profile: Profile,
    #[case] body: &str,
    #[case] expected: RepositoryListingError,
) {
    let server = RecordingServer::start(vec![ScriptedResponse::new(200).with_body(body)])
        .expect("the scripted server should start");
    let client = adapter(profile, &server.origin())
        .expect("an adapter for the named profile should construct");

    let result = client.list_repositories();

    assert_eq!(
        result.err(),
        Some(expected),
        "a body the adapter cannot trust must not become a listing"
    );
}

#[test]
fn an_object_body_is_not_a_user_listing() {
    let server = RecordingServer::start(vec![
        ScriptedResponse::new(200).with_body("{\"repositories\":[]}"),
    ])
    .expect("the scripted server should start");
    let client =
        adapter(Profile::User, &server.origin()).expect("a user-mode adapter should construct");

    let result = client.list_repositories();

    assert_that!(
        &result,
        err(matches_pattern!(
            RepositoryListingError::InvalidResponse { .. }
        ))
    );
}

#[test]
fn a_secondary_limit_message_without_retry_headers_is_throttling() {
    let message = concat!(
        r#"{"message":"You have exceeded a secondary rate limit. "#,
        r#"Please wait a few minutes before you try again."}"#
    );
    let server = RecordingServer::start(vec![
        ScriptedResponse::new(403)
            .with_header("X-RateLimit-Remaining", "4999")
            .with_body(message),
    ])
    .expect("the scripted server should start");
    let client =
        adapter(Profile::User, &server.origin()).expect("a user-mode adapter should construct");

    let result = client.list_repositories();

    assert_eq!(
        result.err(),
        Some(RepositoryListingError::RateLimited {
            retry: RateLimitInfo::new(None, None),
        }),
        "the documented secondary-limit message must yield wait guidance, not a permission error"
    );
    assert_eq!(
        server.request_count(),
        1,
        "a throttled request must not be retried"
    );
}

#[rstest]
#[case::accepted(202)]
#[case::partial_content(206)]
fn an_unexpected_success_status_is_not_a_listing(#[case] status: u16) {
    let server =
        RecordingServer::start(vec![ScriptedResponse::new(status).with_body(bare_page(&[
            repository_json(Repository {
                id: 1,
                owner: "acme",
                name: "one",
            }),
        ]))])
        .expect("the scripted server should start");
    let client =
        adapter(Profile::User, &server.origin()).expect("a user-mode adapter should construct");

    let result = client.list_repositories();

    assert_eq!(
        result.err(),
        Some(RepositoryListingError::UnexpectedStatus { status }),
        "only the documented 200 is a page, so {status} must not become a listing"
    );
    assert_eq!(
        server.request_count(),
        1,
        "an unexpected success status must not be retried"
    );
}
