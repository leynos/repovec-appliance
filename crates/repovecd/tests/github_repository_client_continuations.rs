//! Continuation selection and confinement tests for the listing adapter.
//!
//! A continuation is untrusted input that would otherwise carry the credential
//! to a host of its choosing. These tests script hostile links and assert both
//! the typed failure and the absence of a request that could have leaked it.

mod common;

use std::time::Duration;

use common::{installation_client::installation_client, user_client::user_client};
use googletest::prelude::*;
use pretty_assertions::assert_eq;
use repovec_core::repository_listing::RepositoryListing;
use repovec_test_helpers::{
    recording_http::{ORIGIN_TOKEN, RecordingServer, ScriptedResponse},
    repository_listing_wire::{bare_page, envelope_page, link_header, repository_json, user_page},
};
use repovecd::github_repository_client::ListingLimits;
use rstest::rstest;

/// The request deadline these tests allow themselves.
const TEST_TIMEOUT: Duration = Duration::from_secs(5);

/// Builds a single-record page whose `Link` header names `target`.
fn page_with_next(target: &str) -> ScriptedResponse {
    ScriptedResponse::new(200)
        .with_header("Link", &link_header(target, "next"))
        .with_body(bare_page(&[repository_json(1, "acme", "one")]))
}

/// Builds a single-record enveloped page whose `Link` header names `target`.
fn envelope_page_with_next(target: &str) -> ScriptedResponse {
    ScriptedResponse::new(200)
        .with_header("Link", &link_header(target, "next"))
        .with_body(envelope_page(&[repository_json(1, "acme", "one")]))
}

/// Builds a terminal single-record enveloped page.
fn terminal_envelope_page() -> ScriptedResponse {
    ScriptedResponse::new(200).with_body(envelope_page(&[repository_json(2, "acme", "two")]))
}

/// Builds a terminal single-record page.
fn terminal_page() -> ScriptedResponse {
    ScriptedResponse::new(200).with_body(bare_page(&[repository_json(2, "acme", "two")]))
}

#[test]
fn two_next_targets_are_rejected_before_a_second_request() {
    let header = format!(
        "{first}, {second}",
        first = link_header(&user_page(2), "next"),
        second = link_header(&user_page(3), "next")
    );
    let server = RecordingServer::start(vec![
        ScriptedResponse::new(200)
            .with_header("Link", &header)
            .with_body(bare_page(&[repository_json(1, "acme", "one")])),
        terminal_page(),
    ])
    .expect("the scripted server should start");
    let client = user_client(&server.origin(), ListingLimits::defaults(), TEST_TIMEOUT)
        .expect("a user-mode adapter should construct");

    let result = client.list_repositories();

    assert_that!(
        &result,
        err(matches_pattern!(
            repovec_core::repository_listing::RepositoryListingError::InvalidContinuation { .. }
        ))
    );
    assert_eq!(
        server.request_count(),
        1,
        "an ambiguous continuation must be refused before it is requested"
    );
}

#[test]
fn an_off_origin_continuation_never_reaches_the_second_server() {
    let elsewhere = RecordingServer::start(vec![terminal_page()])
        .expect("the second scripted server should start");
    let foreign = format!(
        "{elsewhere_origin}{target}",
        elsewhere_origin = elsewhere.origin(),
        target = user_page(2)
    );
    let server = RecordingServer::start(vec![page_with_next(&foreign), terminal_page()])
        .expect("the scripted server should start");
    let client = user_client(&server.origin(), ListingLimits::defaults(), TEST_TIMEOUT)
        .expect("a user-mode adapter should construct");

    let result = client.list_repositories();

    assert_that!(
        &result,
        err(matches_pattern!(
            repovec_core::repository_listing::RepositoryListingError::InvalidContinuation { .. }
        ))
    );
    assert_eq!(
        elsewhere.request_count(),
        0,
        "a credential must never be sent to a host the listing did not name"
    );
    assert_eq!(
        server.request_count(),
        1,
        "the rejected continuation must not be requested"
    );
}

#[rstest]
#[case::changed_path("/installation/repositories?per_page=100&page=2")]
#[case::added_filter(
    "/user/repos?visibility=all&affiliation=owner,collaborator,organization_member&sort=full_name&\
     direction=asc&per_page=100&type=all&page=2"
)]
#[case::changed_filter(
    "/user/repos?visibility=private&affiliation=owner,collaborator,organization_member&\
     sort=full_name&direction=asc&per_page=100&page=2"
)]
#[case::dropped_filter("/user/repos?per_page=100&page=2")]
#[case::fragment(
    "/user/repos?visibility=all&affiliation=owner,collaborator,organization_member&sort=full_name&\
     direction=asc&per_page=100&page=2#section"
)]
#[case::userinfo(
    "{origin_scheme}//user:secret@127.0.0.1:1/user/repos?visibility=all&affiliation=owner,\
     collaborator,organization_member&sort=full_name&direction=asc&per_page=100&page=2"
)]
fn a_continuation_outside_the_listing_is_rejected(#[case] suffix: &str) {
    let target = format!("{ORIGIN_TOKEN}{suffix}").replace("{origin_scheme}", "http:");
    let server = RecordingServer::start(vec![page_with_next(&target), terminal_page()])
        .expect("the scripted server should start");
    let client = user_client(&server.origin(), ListingLimits::defaults(), TEST_TIMEOUT)
        .expect("a user-mode adapter should construct");

    let result = client.list_repositories();

    assert_that!(
        &result,
        err(matches_pattern!(
            repovec_core::repository_listing::RepositoryListingError::InvalidContinuation { .. }
        ))
    );
    assert_eq!(
        server.request_count(),
        1,
        "a continuation that leaves the listing must not be requested"
    );
}

#[test]
fn reordered_fixed_filters_are_still_confined() {
    let target = format!(
        "{ORIGIN_TOKEN}/user/repos?per_page=100&sort=full_name&visibility=all&affiliation=owner,\
         collaborator,organization_member&direction=asc&page=2"
    );
    let server = RecordingServer::start(vec![page_with_next(&target), terminal_page()])
        .expect("the scripted server should start");
    let client = user_client(&server.origin(), ListingLimits::defaults(), TEST_TIMEOUT)
        .expect("a user-mode adapter should construct");

    let repositories = client
        .list_repositories()
        .expect("query order alone must not look like a changed filter");

    assert_eq!(
        repositories.len(),
        2,
        "a reordered fixed query is the same listing"
    );
}

#[test]
fn a_repeated_page_is_a_cycle() {
    let second = user_page(2);
    let server = RecordingServer::start(vec![page_with_next(&second), page_with_next(&second)])
        .expect("the scripted server should start");
    let client = user_client(&server.origin(), ListingLimits::defaults(), TEST_TIMEOUT)
        .expect("a user-mode adapter should construct");

    let result = client.list_repositories();

    assert_eq!(
        result.err(),
        Some(repovec_core::repository_listing::RepositoryListingError::CycleDetected),
        "a repeated request must be reported as a cycle"
    );
    assert_eq!(
        server.request_count(),
        2,
        "a cycle must be detected instead of requested again"
    );
}

#[test]
fn a_two_page_cycle_is_detected() {
    let second = user_page(2);
    let third = user_page(3);
    let server = RecordingServer::start(vec![
        page_with_next(&second),
        page_with_next(&third),
        page_with_next(&second),
    ])
    .expect("the scripted server should start");
    let client = user_client(&server.origin(), ListingLimits::defaults(), TEST_TIMEOUT)
        .expect("a user-mode adapter should construct");

    let result = client.list_repositories();

    assert_eq!(
        result.err(),
        Some(repovec_core::repository_listing::RepositoryListingError::CycleDetected),
        "a repeated request must be reported as a cycle"
    );
    assert_eq!(
        server.request_count(),
        3,
        "the repeated page must be recognized rather than fetched"
    );
}

#[test]
fn a_redirect_is_not_followed() {
    let elsewhere = RecordingServer::start(vec![terminal_page()])
        .expect("the second scripted server should start");
    let location = format!("{}{}", elsewhere.origin(), user_page(2));
    let server = RecordingServer::start(vec![
        ScriptedResponse::new(302).with_header("Location", &location),
        terminal_page(),
    ])
    .expect("the scripted server should start");
    let client = user_client(&server.origin(), ListingLimits::defaults(), TEST_TIMEOUT)
        .expect("a user-mode adapter should construct");

    let result = client.list_repositories();

    assert_eq!(
        result.err(),
        Some(
            repovec_core::repository_listing::RepositoryListingError::UnexpectedStatus {
                status: 302
            }
        ),
        "a redirect is an unexpected status because none are followed"
    );
    assert_eq!(
        elsewhere.request_count(),
        0,
        "a redirect must not move the credential to another server"
    );
}

#[test]
fn an_empty_page_with_a_continuation_still_continues() {
    let second = user_page(2);
    let server = RecordingServer::start(vec![
        ScriptedResponse::new(200)
            .with_header("Link", &link_header(&second, "next"))
            .with_body("[]"),
        terminal_page(),
    ])
    .expect("the scripted server should start");
    let client = user_client(&server.origin(), ListingLimits::defaults(), TEST_TIMEOUT)
        .expect("a user-mode adapter should construct");

    let repositories = client
        .list_repositories()
        .expect("an empty page with a continuation is not the end");

    assert_eq!(
        repositories.len(),
        1,
        "the record after the empty page must be returned"
    );
    assert_eq!(
        server.request_count(),
        2,
        "completion is decided by the Link header, not by page size"
    );
}
#[test]
fn an_installation_continuation_may_not_change_to_the_user_endpoint() {
    let target = format!("{ORIGIN_TOKEN}/user/repos?per_page=100&page=2");
    let server = RecordingServer::start(vec![
        envelope_page_with_next(&target),
        terminal_envelope_page(),
    ])
    .expect("the scripted server should start");
    let client = installation_client(&server.origin(), ListingLimits::defaults(), TEST_TIMEOUT)
        .expect("an installation-mode adapter should construct");

    let result = client.list_repositories();

    assert_that!(
        &result,
        err(matches_pattern!(
            repovec_core::repository_listing::RepositoryListingError::InvalidContinuation { .. }
        ))
    );
    assert_eq!(
        server.request_count(),
        1,
        "a continuation that changes the endpoint must not be requested"
    );
}
