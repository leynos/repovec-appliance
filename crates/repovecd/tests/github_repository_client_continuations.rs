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
use repovec_core::repository_listing::{RepositoryListing, RepositoryListingError};
use repovec_test_helpers::{
    recording_http::{ORIGIN_AUTHORITY_TOKEN, ORIGIN_TOKEN, RecordingServer, ScriptedResponse},
    repository_listing_wire::{
        LinkRelation,
        Repository,
        bare_page,
        envelope_page,
        link_header,
        repository_json,
        user_page,
    },
};
use repovecd::github_repository_client::{GitHubRepositoryClient, ListingLimits};
use rstest::rstest;

/// The request deadline these tests allow themselves.
const TEST_TIMEOUT: Duration = Duration::from_secs(5);

/// Which credential kind an adapter under test presents.
#[derive(Clone, Copy, Debug)]
enum Profile {
    /// A user credential, whose pages are bare arrays.
    User,
    /// An installation credential, whose pages are enveloped.
    Installation,
}

/// Returns an adapter for the profile at a loopback origin.
fn client_for(
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

/// Returns the first page the profile serves, naming `target` as its next link.
fn opening_page(profile: Profile, target: &str) -> ScriptedResponse {
    match profile {
        Profile::User => page_with_next(target),
        Profile::Installation => envelope_page_with_next(target),
    }
}

/// Returns the terminal page the profile serves.
fn last_page(profile: Profile) -> ScriptedResponse {
    match profile {
        Profile::User => terminal_page(),
        Profile::Installation => terminal_envelope_page(),
    }
}

/// Builds a single-record page whose `Link` header names `target`.
fn page_with_next(target: &str) -> ScriptedResponse {
    ScriptedResponse::new(200)
        .with_header("Link", &link_header(target, LinkRelation::Next))
        .with_body(bare_page(&[repository_json(Repository {
            id: 1,
            owner: "acme",
            name: "one",
        })]))
}

/// Builds a single-record enveloped page whose `Link` header names `target`.
fn envelope_page_with_next(target: &str) -> ScriptedResponse {
    ScriptedResponse::new(200)
        .with_header("Link", &link_header(target, LinkRelation::Next))
        .with_body(envelope_page(&[repository_json(Repository {
            id: 1,
            owner: "acme",
            name: "one",
        })]))
}

/// Builds a terminal single-record enveloped page.
fn terminal_envelope_page() -> ScriptedResponse {
    ScriptedResponse::new(200).with_body(envelope_page(&[repository_json(Repository {
        id: 2,
        owner: "acme",
        name: "two",
    })]))
}

/// Builds a terminal single-record page.
fn terminal_page() -> ScriptedResponse {
    ScriptedResponse::new(200).with_body(bare_page(&[repository_json(Repository {
        id: 2,
        owner: "acme",
        name: "two",
    })]))
}

#[test]
fn a_mixed_case_next_relation_still_continues_the_listing() {
    let header = format!("<{}>; Rel=\"NeXt\"", user_page(2));
    let server = RecordingServer::start(vec![
        ScriptedResponse::new(200)
            .with_header("Link", &header)
            .with_body(bare_page(&[repository_json(Repository {
                id: 1,
                owner: "acme",
                name: "one",
            })])),
        terminal_page(),
    ])
    .expect("the scripted server should start");
    let client = user_client(&server.origin(), ListingLimits::defaults(), TEST_TIMEOUT)
        .expect("a user-mode adapter should construct");

    let repositories = client
        .list_repositories()
        .expect("a case-insensitive relation names an ordinary continuation");

    assert_eq!(
        repositories.len(),
        2,
        "the second page must be requested and returned"
    );
    assert_eq!(
        server.request_count(),
        2,
        "a mixed-case next relation must not end the listing early"
    );
}

#[test]
fn two_next_targets_are_rejected_before_a_second_request() {
    let header = format!(
        "{first}, {second}",
        first = link_header(&user_page(2), LinkRelation::Next),
        second = link_header(&user_page(3), LinkRelation::Next)
    );
    let server = RecordingServer::start(vec![
        ScriptedResponse::new(200)
            .with_header("Link", &header)
            .with_body(bare_page(&[repository_json(Repository {
                id: 1,
                owner: "acme",
                name: "one",
            })])),
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

#[test]
fn the_case_placeholders_match_the_recording_server_constants() {
    assert_eq!(
        (ORIGIN_TOKEN, ORIGIN_AUTHORITY_TOKEN),
        ("{origin}", "{origin_authority}"),
        "the case templates below spell the fixture placeholders literally"
    );
}

// Each case names the whole continuation target, so the placeholders show
// where the server's own origin belongs. The userinfo case names its
// authority explicitly: user information belongs before the authority, which
// prefixing the origin cannot express.
#[rstest]
#[case::changed_path(
    Profile::User,
    "{origin}/installation/repositories?per_page=100&page=2"
)]
#[case::added_filter(
    Profile::User,
    "{origin}/user/repos?visibility=all&affiliation=owner,collaborator,organization_member&\
     sort=full_name&direction=asc&per_page=100&type=all&page=2"
)]
#[case::changed_filter(
    Profile::User,
    "{origin}/user/repos?visibility=private&affiliation=owner,collaborator,organization_member&\
     sort=full_name&direction=asc&per_page=100&page=2"
)]
#[case::dropped_filter(Profile::User, "{origin}/user/repos?per_page=100&page=2")]
#[case::fragment(
    Profile::User,
    "{origin}/user/repos?visibility=all&affiliation=owner,collaborator,organization_member&\
     sort=full_name&direction=asc&per_page=100&page=2#section"
)]
#[case::userinfo(
    Profile::User,
    "http://user:secret@{origin_authority}/user/repos?visibility=all&affiliation=owner,\
     collaborator,organization_member&sort=full_name&direction=asc&per_page=100&page=2"
)]
#[case::installation_changes_endpoint(
    Profile::Installation,
    "{origin}/user/repos?per_page=100&page=2"
)]
fn a_continuation_outside_the_listing_is_rejected(#[case] profile: Profile, #[case] target: &str) {
    let server = RecordingServer::start(vec![opening_page(profile, target), last_page(profile)])
        .expect("the scripted server should start");
    let client = client_for(profile, &server.origin())
        .expect("an adapter for the named profile should construct");

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
        "a continuation that leaves the listing must not be requested for {profile:?}"
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

#[rstest]
#[case::immediate_repeat(&[2, 2], 2)]
#[case::two_page_cycle(&[2, 3, 2], 3)]
fn a_repeated_page_is_a_cycle(#[case] pages: &[u32], #[case] expected_requests: usize) {
    let script = pages
        .iter()
        .map(|page| page_with_next(&user_page(*page)))
        .collect::<Vec<_>>();
    let server = RecordingServer::start(script).expect("the scripted server should start");
    let client = user_client(&server.origin(), ListingLimits::defaults(), TEST_TIMEOUT)
        .expect("a user-mode adapter should construct");

    let result = client.list_repositories();

    assert_eq!(
        result.err(),
        Some(repovec_core::repository_listing::RepositoryListingError::CycleDetected),
        "a repeated request must be reported as a cycle for {pages:?}"
    );
    assert_eq!(
        server.request_count(),
        expected_requests,
        "the repeated page must be recognized rather than fetched for {pages:?}"
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
            .with_header("Link", &link_header(&second, LinkRelation::Next))
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
