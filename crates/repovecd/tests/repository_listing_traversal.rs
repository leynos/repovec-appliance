//! Repository-agnostic traversal tests.
//!
//! The shared traversal is meant to serve every GitHub REST listing, so it must
//! carry no repository knowledge. These tests run it through a third profile
//! with a different path, a different fixed query, and a different envelope
//! key, then assert that a body holding the repository envelope under this
//! profile fails. Traversal code that hard-coded `/user/repos` or the
//! `repositories` key would pass the adapter tests and fail here.

// The traversal tests need a raw HTTP client rather than the adapter fixtures,
// so they include the client module directly. Declaring `common` here would
// also compile the adapter constructors, which no test in this binary uses.
#[path = "common/blocking_client.rs"]
mod blocking_client;

use std::time::Duration;

use blocking_client::blocking_client;
use pretty_assertions::assert_eq;
use repovec_core::{github_oauth::AccessToken, repository_listing::RepositoryListingError};
use repovec_test_helpers::{
    recording_http::{ORIGIN_TOKEN, RecordedRequest, RecordingServer, ScriptedResponse},
    repository_listing_wire::{LinkRelation, USER_TOKEN, link_header},
};
use repovecd::github_repository_client::{
    GitHubCredential,
    ListingLimits,
    TestPageItems,
    TestProfile,
    TestTraversal,
    request_url,
    traverse,
};
use serde::Deserialize;

/// The request deadline these tests allow themselves.
const TEST_TIMEOUT: Duration = Duration::from_secs(5);

/// A record type no repository endpoint uses.
#[derive(Debug, Deserialize, PartialEq)]
struct Widget {
    /// Numeric widget identifier.
    id: u64,
    /// Human-readable widget label.
    label: String,
}

/// A profile no repository endpoint uses: another path, filters, and key.
const WIDGETS: TestProfile = TestProfile {
    path: "/widgets",
    fixed_query: &[("kind", "all"), ("per_page", "50")],
    items: TestPageItems::Envelope("items"),
};

/// Builds the request target of the second widget page.
fn second_page() -> String { "/widgets?kind=all&per_page=50&page=2".to_owned() }

/// Builds the credential every test presents.
fn credential() -> GitHubCredential {
    GitHubCredential::User(AccessToken::new(USER_TOKEN, ["repo"]))
}

#[test]
fn a_non_repository_profile_pages_without_repository_knowledge() {
    let server = RecordingServer::start(vec![
        ScriptedResponse::new(200)
            .with_header("Link", &link_header(&second_page(), LinkRelation::Next))
            .with_body(r#"{"items":[{"id":1,"label":"first"},{"id":2,"label":"second"}]}"#),
        ScriptedResponse::new(200).with_body(r#"{"items":[{"id":3,"label":"third"}]}"#),
    ])
    .expect("the scripted server should start");
    let initial = request_url(&server.origin(), WIDGETS.path, WIDGETS.fixed_query)
        .expect("a loopback origin should parse");
    let http = blocking_client(TEST_TIMEOUT).expect("a test HTTP client should build");

    let evidence = traverse::<Widget>(
        TestTraversal {
            http: &http,
            credential: &credential(),
            profile: WIDGETS,
            limits: ListingLimits::defaults(),
        },
        initial,
    )
    .expect("a profile with its own path and key should page normally");

    assert_eq!(
        evidence.records,
        vec![
            Widget {
                id: 1,
                label: "first".to_owned()
            },
            Widget {
                id: 2,
                label: "second".to_owned()
            },
            Widget {
                id: 3,
                label: "third".to_owned()
            },
        ],
        "every page must be returned in request order"
    );
    assert_eq!(evidence.pages, 2, "both pages must be counted");
    let requests = server.requests();
    assert_eq!(
        requests.first().map(RecordedRequest::path),
        Some("/widgets"),
        "the traversal must request the profile's path, not a repository path"
    );
    assert_eq!(
        requests.get(1).map(|request| request.target.as_str()),
        Some(second_page().as_str()),
        "the continuation must follow the profile's own query"
    );
}

#[test]
fn the_profile_key_is_honoured_rather_than_the_repository_key() {
    let server = RecordingServer::start(vec![
        ScriptedResponse::new(200).with_body(r#"{"repositories":[{"id":1,"label":"first"}]}"#),
    ])
    .expect("the scripted server should start");
    let initial = request_url(&server.origin(), WIDGETS.path, WIDGETS.fixed_query)
        .expect("a loopback origin should parse");
    let http = blocking_client(TEST_TIMEOUT).expect("a test HTTP client should build");

    let result = traverse::<Widget>(
        TestTraversal {
            http: &http,
            credential: &credential(),
            profile: WIDGETS,
            limits: ListingLimits::defaults(),
        },
        initial,
    );

    assert_eq!(
        result.err(),
        Some(RepositoryListingError::InvalidResponse {
            reason: "page body is missing its records",
        }),
        "the repository envelope key must not satisfy another profile"
    );
}

#[test]
fn a_bare_array_profile_reads_a_bare_array() {
    let profile = TestProfile {
        path: "/widgets",
        fixed_query: &[("per_page", "50")],
        items: TestPageItems::BareArray,
    };
    let server = RecordingServer::start(vec![
        ScriptedResponse::new(200).with_body(r#"[{"id":1,"label":"first"}]"#),
    ])
    .expect("the scripted server should start");
    let initial = request_url(&server.origin(), profile.path, profile.fixed_query)
        .expect("a loopback origin should parse");
    let http = blocking_client(TEST_TIMEOUT).expect("a test HTTP client should build");

    let evidence = traverse::<Widget>(
        TestTraversal {
            http: &http,
            credential: &credential(),
            profile,
            limits: ListingLimits::defaults(),
        },
        initial,
    )
    .expect("a bare-array profile should decode a bare array");

    assert_eq!(
        evidence.records.len(),
        1,
        "the array variant must remain reachable with a third profile"
    );
    assert_eq!(evidence.pages, 1, "one terminal page is one page");
}

#[test]
fn a_continuation_outside_the_profile_is_still_confined() {
    let foreign = format!("{ORIGIN_TOKEN}/user/repos?per_page=50&page=2");
    let server = RecordingServer::start(vec![
        ScriptedResponse::new(200)
            .with_header("Link", &link_header(&foreign, LinkRelation::Next))
            .with_body(r#"{"items":[]}"#),
    ])
    .expect("the scripted server should start");
    let initial = request_url(&server.origin(), WIDGETS.path, WIDGETS.fixed_query)
        .expect("a loopback origin should parse");
    let http = blocking_client(TEST_TIMEOUT).expect("a test HTTP client should build");

    let result = traverse::<Widget>(
        TestTraversal {
            http: &http,
            credential: &credential(),
            profile: WIDGETS,
            limits: ListingLimits::defaults(),
        },
        initial,
    );

    assert_eq!(
        result.err(),
        Some(RepositoryListingError::InvalidContinuation {
            reason: "next link changes the listing path",
        }),
        "confinement belongs to the traversal, not to one profile"
    );
    assert_eq!(
        server.request_count(),
        1,
        "the off-profile continuation must not be requested"
    );
}
