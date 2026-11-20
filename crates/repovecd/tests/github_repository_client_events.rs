//! Run-event tests for the GitHub repository listing adapter.
//!
//! A run reports exactly one terminal event: completion only after every
//! record has been validated and written, or a failure carrying the counts
//! accepted before it. Each test captures the adapter's logs and asserts the
//! event, the counts it carries, and that no credential or repository name
//! reaches them.

mod common;

use std::time::Duration;

use common::{installation_client::installation_client, user_client::user_client};
use repovec_core::repository_listing::RepositoryListing;
use repovec_test_helpers::{
    recording_http::{RecordingServer, ScriptedResponse},
    repository_listing_wire::{
        INSTALLATION_TOKEN,
        LinkRelation,
        Repository,
        USER_TOKEN,
        bare_page,
        envelope_page,
        link_header,
        repository_json,
        user_page,
    },
};
use repovecd::github_repository_client::ListingLimits;

/// The request deadline the event tests allow themselves.
const TEST_TIMEOUT: Duration = Duration::from_secs(5);

#[test]
fn a_completed_listing_logs_counts_without_naming_repositories_or_secrets() {
    let server = RecordingServer::start(vec![ScriptedResponse::new(200).with_body(bare_page(&[
        repository_json(Repository {
            id: 1,
            owner: "acme",
            name: "confidential-project",
        }),
    ]))])
    .expect("the scripted server should start");
    let client = user_client(&server.origin(), ListingLimits::defaults(), TEST_TIMEOUT)
        .expect("a user-mode adapter should construct");

    let (result, logs) = repovec_test_helpers::capture_logs(|| client.list_repositories())
        .expect("capturing logs should succeed");

    assert!(
        result.is_ok(),
        "the listing should complete, got {result:?}"
    );
    assert!(
        logs.contains("repository listing completed"),
        "a completed listing must be observable: {logs}"
    );
    assert!(
        logs.contains("pages=1") && logs.contains("records=1"),
        "the completion event must carry the accepted page and record counts: {logs}"
    );
    assert!(
        !logs.contains(USER_TOKEN),
        "logs must never contain the credential: {logs}"
    );
    assert!(
        !logs.contains("confidential-project"),
        "logs must not record repository names by default: {logs}"
    );
}

#[test]
fn a_record_outside_the_domain_fails_without_a_completion_event() {
    let server = RecordingServer::start(vec![ScriptedResponse::new(200).with_body(bare_page(&[
        repository_json(Repository {
            id: 1,
            owner: "acme",
            name: "",
        }),
    ]))])
    .expect("the scripted server should start");
    let client = user_client(&server.origin(), ListingLimits::defaults(), TEST_TIMEOUT)
        .expect("a user-mode adapter should construct");

    let (result, logs) = repovec_test_helpers::capture_logs(|| client.list_repositories())
        .expect("capturing logs should succeed");

    assert!(
        result.is_err(),
        "a record the domain rejects must fail the listing, got {result:?}"
    );
    assert!(
        !logs.contains("repository listing completed"),
        "a rejected record must not be reported as a completed listing: {logs}"
    );
    assert!(
        logs.contains("repository listing failed"),
        "the rejection must be observable: {logs}"
    );
    assert!(
        logs.contains("pages=1") && logs.contains("records=1"),
        "the failure event must carry the accepted counts: {logs}"
    );
}

#[test]
fn an_invalid_continuation_on_the_first_page_accepts_no_work() {
    let invalid = "</user/repos?page=2>; rel=\"next\"";
    let server = RecordingServer::start(vec![
        ScriptedResponse::new(200)
            .with_header("Link", invalid)
            .with_body(bare_page(&[repository_json(Repository {
                id: 1,
                owner: "acme",
                name: "one",
            })])),
    ])
    .expect("the scripted server should start");
    let client = user_client(&server.origin(), ListingLimits::defaults(), TEST_TIMEOUT)
        .expect("a user-mode adapter should construct");

    let (result, logs) = repovec_test_helpers::capture_logs(|| client.list_repositories())
        .expect("capturing logs should succeed");

    assert!(
        result.is_err(),
        "an invalid continuation must fail the listing, got {result:?}"
    );
    assert!(
        logs.contains("pages=0") && logs.contains("records=0"),
        "a page whose continuation is invalid must not be counted as accepted: {logs}"
    );
}

#[test]
fn an_invalid_continuation_after_one_page_counts_only_that_page() {
    let second = user_page(2);
    let invalid = "</user/repos?page=3>; rel=\"next\"";
    let server = RecordingServer::start(vec![
        ScriptedResponse::new(200)
            .with_header("Link", &link_header(&second, LinkRelation::Next))
            .with_body(bare_page(&[repository_json(Repository {
                id: 1,
                owner: "acme",
                name: "one",
            })])),
        ScriptedResponse::new(200)
            .with_header("Link", invalid)
            .with_body(bare_page(&[repository_json(Repository {
                id: 2,
                owner: "acme",
                name: "two",
            })])),
    ])
    .expect("the scripted server should start");
    let client = user_client(&server.origin(), ListingLimits::defaults(), TEST_TIMEOUT)
        .expect("a user-mode adapter should construct");

    let (result, logs) = repovec_test_helpers::capture_logs(|| client.list_repositories())
        .expect("capturing logs should succeed");

    assert!(
        result.is_err(),
        "the second page's invalid continuation must fail the listing, got {result:?}"
    );
    assert!(
        logs.contains("pages=1") && logs.contains("records=1"),
        "only the page whose continuation validated may be counted: {logs}"
    );
}

#[test]
fn a_completed_installation_listing_logs_its_counts() {
    let server = RecordingServer::start(vec![ScriptedResponse::new(200).with_body(envelope_page(
        &[repository_json(Repository {
            id: 1,
            owner: "acme",
            name: "confidential-project",
        })],
    ))])
    .expect("the scripted server should start");
    let client = installation_client(&server.origin(), ListingLimits::defaults(), TEST_TIMEOUT)
        .expect("an installation-mode adapter should construct");

    let (result, logs) = repovec_test_helpers::capture_logs(|| client.list_repositories())
        .expect("capturing logs should succeed");

    assert!(
        result.is_ok(),
        "the enveloped listing should complete, got {result:?}"
    );
    assert!(
        logs.contains("pages=1") && logs.contains("records=1"),
        "the enveloped completion event must carry the accepted page and record counts: {logs}"
    );
    assert!(
        !logs.contains(INSTALLATION_TOKEN),
        "logs must never contain the credential: {logs}"
    );
    assert!(
        !logs.contains("confidential-project"),
        "logs must not record repository names by default: {logs}"
    );
}
