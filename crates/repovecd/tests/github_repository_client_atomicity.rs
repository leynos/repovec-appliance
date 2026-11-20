//! Page-atomicity tests for the GitHub repository listing adapter.
//!
//! A listing either completes or fails: a caller may never receive the pages
//! that were accepted before a later failure. Each test scripts one accepted
//! page followed by a failing one and asserts the typed error, the exact
//! request count, and the absence of any listing value.

mod common;

use std::time::Duration;

use common::{installation_client::installation_client, user_client::user_client};
use pretty_assertions::assert_eq;
use repovec_core::repository_listing::{RateLimitInfo, RepositoryListing, RepositoryListingError};
use repovec_test_helpers::{
    recording_http::{RecordingServer, ScriptedResponse},
    repository_listing_wire::{
        LinkRelation,
        Repository,
        bare_page,
        envelope_page,
        installation_page,
        link_header,
        repository_json,
        user_page,
    },
};
use repovecd::github_repository_client::ListingLimits;

/// The request deadline the atomicity tests allow themselves.
const TEST_TIMEOUT: Duration = Duration::from_secs(5);

/// The deadline used by the timeout case, well below the scripted delay.
const EXPIRING_TIMEOUT: Duration = Duration::from_millis(500);

/// The delay the timeout case scripts, well above its request deadline.
const SLOW_RESPONSE: Duration = Duration::from_secs(5);

#[test]
fn a_failure_on_a_later_page_returns_no_prefix() {
    let second = user_page(2);
    let server = RecordingServer::start(vec![
        ScriptedResponse::new(200)
            .with_header("Link", &link_header(&second, LinkRelation::Next))
            .with_body(bare_page(&[
                repository_json(Repository {
                    id: 1,
                    owner: "acme",
                    name: "one",
                }),
                repository_json(Repository {
                    id: 2,
                    owner: "acme",
                    name: "two",
                }),
            ])),
        ScriptedResponse::new(500),
    ])
    .expect("the scripted server should start");
    let client = user_client(&server.origin(), ListingLimits::defaults(), TEST_TIMEOUT)
        .expect("a user-mode adapter should construct");

    let result = client.list_repositories();

    assert_eq!(
        result.err(),
        Some(RepositoryListingError::UnexpectedStatus { status: 500 }),
        "two accepted records must not survive a failed third page as success"
    );
    assert_eq!(
        server.request_count(),
        2,
        "the failure must end the traversal"
    );
}

#[test]
fn a_malformed_record_after_accepted_pages_returns_no_prefix() {
    let second = user_page(2);
    let server = RecordingServer::start(vec![
        ScriptedResponse::new(200)
            .with_header("Link", &link_header(&second, LinkRelation::Next))
            .with_body(bare_page(&[repository_json(Repository {
                id: 1,
                owner: "acme",
                name: "one",
            })])),
        ScriptedResponse::new(200).with_body(bare_page(&[repository_json(Repository {
            id: 2,
            owner: "acme",
            name: "",
        })])),
    ])
    .expect("the scripted server should start");
    let client = user_client(&server.origin(), ListingLimits::defaults(), TEST_TIMEOUT)
        .expect("a user-mode adapter should construct");

    let result = client.list_repositories();

    assert_eq!(
        result.err(),
        Some(RepositoryListingError::InvalidResponse {
            reason: "repository name is invalid",
        }),
        "validation must cover every page, not only the first"
    );
    assert_eq!(
        server.request_count(),
        2,
        "the failing page must not be retried"
    );
}

#[test]
fn a_rate_limited_page_after_accepted_pages_returns_no_prefix() {
    let second = user_page(2);
    let server = RecordingServer::start(vec![
        ScriptedResponse::new(200)
            .with_header("Link", &link_header(&second, LinkRelation::Next))
            .with_body(bare_page(&[repository_json(Repository {
                id: 1,
                owner: "acme",
                name: "one",
            })])),
        ScriptedResponse::new(403)
            .with_header("X-RateLimit-Remaining", "0")
            .with_header("X-RateLimit-Reset", "1700000000"),
    ])
    .expect("the scripted server should start");
    let client = user_client(&server.origin(), ListingLimits::defaults(), TEST_TIMEOUT)
        .expect("a user-mode adapter should construct");

    let result = client.list_repositories();

    assert_eq!(
        result.err(),
        Some(RepositoryListingError::RateLimited {
            retry: RateLimitInfo::new(None, Some(1_700_000_000)),
        }),
        "a throttled later page must fail the listing with its retry metadata"
    );
    assert_eq!(
        server.request_count(),
        2,
        "the throttled page must end the traversal without a retry"
    );
}

#[test]
fn a_timed_out_page_after_accepted_pages_returns_no_prefix() {
    let second = user_page(2);
    let server = RecordingServer::start(vec![
        ScriptedResponse::new(200)
            .with_header("Link", &link_header(&second, LinkRelation::Next))
            .with_body(bare_page(&[repository_json(Repository {
                id: 1,
                owner: "acme",
                name: "one",
            })])),
        ScriptedResponse::new(200).with_delay(SLOW_RESPONSE),
    ])
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
        "a later page that outlives its deadline must fail the whole listing"
    );
    assert_eq!(
        server.request_count(),
        2,
        "the timed-out page must end the traversal without a retry"
    );
}

#[test]
fn an_installation_failure_on_a_later_page_returns_no_prefix() {
    let second = installation_page(2);
    let server = RecordingServer::start(vec![
        ScriptedResponse::new(200)
            .with_header("Link", &link_header(&second, LinkRelation::Next))
            .with_body(envelope_page(&[repository_json(Repository {
                id: 1,
                owner: "acme",
                name: "one",
            })])),
        ScriptedResponse::new(401),
    ])
    .expect("the scripted server should start");
    let client = installation_client(&server.origin(), ListingLimits::defaults(), TEST_TIMEOUT)
        .expect("an installation-mode adapter should construct");

    let result = client.list_repositories();

    assert_eq!(
        result.err(),
        Some(RepositoryListingError::AuthenticationRejected),
        "the enveloped profile must not return its accepted page either"
    );
    assert_eq!(
        server.request_count(),
        2,
        "the rejected page must end the traversal without a retry"
    );
}
