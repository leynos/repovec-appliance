//! Protocol tests for the GitHub repository listing adapter.
//!
//! Each test scripts one loopback server and asserts the exact requests the
//! real adapter makes, so the evidence is the HTTP the adapter sent rather
//! than a description of what it ought to send. Every credential is a fake.

mod common;

use std::time::Duration;

use common::{installation_client::installation_client, user_client::user_client};
use pretty_assertions::assert_eq;
use repovec_core::repository_listing::RepositoryListing;
use repovec_test_helpers::{
    recording_http::{ORIGIN_TOKEN, RecordingServer, ScriptedResponse},
    repository_listing_wire::{
        INSTALLATION_QUERY,
        INSTALLATION_TOKEN,
        LinkRelation,
        Repository,
        USER_QUERY,
        USER_TOKEN,
        bare_page,
        decode_target,
        envelope_page,
        link_header,
        repository_json,
        user_page,
    },
};
use repovecd::github_repository_client::ListingLimits;
use rstest::rstest;

/// The request deadline the workflow tests allow themselves.
const TEST_TIMEOUT: Duration = Duration::from_secs(5);

#[test]
fn user_mode_sends_the_documented_request() {
    let server = RecordingServer::start(vec![ScriptedResponse::new(200).with_body(bare_page(&[
        repository_json(Repository {
            id: 1,
            owner: "octocat",
            name: "hello-world",
        }),
        repository_json(Repository {
            id: 2,
            owner: "acme",
            name: "service",
        }),
    ]))])
    .expect("the scripted server should start");
    let client = user_client(&server.origin(), ListingLimits::defaults(), TEST_TIMEOUT)
        .expect("a user-mode adapter should construct");

    let repositories = client
        .list_repositories()
        .expect("a single terminal page should complete");

    assert_eq!(
        repositories
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>(),
        vec!["octocat/hello-world".to_owned(), "acme/service".to_owned()],
        "both observations must be returned in page order"
    );
    let requests = server.requests();
    assert_eq!(requests.len(), 1, "exactly one page must be requested");
    let request = requests
        .first()
        .expect("the page request should have been recorded");
    assert_eq!(request.method, "GET", "the listing must never mutate");
    assert_eq!(
        request.path(),
        "/user/repos",
        "the user profile must use the documented path"
    );
    let decoded = decode_target(&request.target).expect("a recorded request target should parse");
    assert_eq!(
        decoded.query,
        USER_QUERY
            .iter()
            .map(|(key, value)| ((*key).to_owned(), (*value).to_owned()))
            .collect::<Vec<_>>(),
        "the fixed query must pin visibility, affiliation, ordering, and size"
    );
    assert_eq!(
        request.header("authorization"),
        Some(format!("Bearer {USER_TOKEN}").as_str()),
        "the user credential must be presented as a bearer token"
    );
    assert_eq!(
        request.header("accept"),
        Some("application/vnd.github+json"),
        "the stable JSON media type must be requested"
    );
    assert_eq!(
        request.header("user-agent"),
        Some("repovecd"),
        "GitHub rejects requests without a product user agent"
    );
    assert_eq!(
        request.header("x-github-api-version"),
        Some("2026-03-10"),
        "the request must pin the API version it was written against"
    );
}

#[test]
fn installation_mode_reads_the_repositories_envelope() {
    let server = RecordingServer::start(vec![ScriptedResponse::new(200).with_body(envelope_page(
        &[
            repository_json(Repository {
                id: 7,
                owner: "acme",
                name: "billing",
            }),
            repository_json(Repository {
                id: 8,
                owner: "acme",
                name: "portal",
            }),
        ],
    ))])
    .expect("the scripted server should start");
    let client = installation_client(&server.origin(), ListingLimits::defaults(), TEST_TIMEOUT)
        .expect("an installation-mode adapter should construct");

    let repositories = client
        .list_repositories()
        .expect("an enveloped page should decode");

    assert_eq!(
        repositories
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>(),
        vec!["acme/billing".to_owned(), "acme/portal".to_owned()],
        "the envelope items must become observations in order"
    );
    let requests = server.requests();
    assert_eq!(requests.len(), 1, "exactly one page must be requested");
    let request = requests
        .first()
        .expect("the page request should have been recorded");
    assert_eq!(
        request.path(),
        "/installation/repositories",
        "the installation profile must use the documented path"
    );
    let decoded = decode_target(&request.target).expect("a recorded request target should parse");
    assert_eq!(
        decoded.query,
        INSTALLATION_QUERY
            .iter()
            .map(|(key, value)| ((*key).to_owned(), (*value).to_owned()))
            .collect::<Vec<_>>(),
        "the installation profile fixes only the page size"
    );
    assert_eq!(
        request.header("authorization"),
        Some(format!("Bearer {INSTALLATION_TOKEN}").as_str()),
        "the installation credential must be presented as a bearer token"
    );
}

#[test]
fn a_short_page_without_a_next_link_completes() {
    let server = RecordingServer::start(vec![ScriptedResponse::new(200).with_body(bare_page(&[
        repository_json(Repository {
            id: 1,
            owner: "acme",
            name: "one",
        }),
    ]))])
    .expect("the scripted server should start");
    let client = user_client(&server.origin(), ListingLimits::defaults(), TEST_TIMEOUT)
        .expect("a user-mode adapter should construct");

    let repositories = client
        .list_repositories()
        .expect("a short page without a next link is terminal");

    assert_eq!(repositories.len(), 1, "the only page must be returned");
    assert_eq!(
        server.request_count(),
        1,
        "completion must not depend on a page being full"
    );
}

#[test]
fn an_absolute_next_link_pages_in_request_order() {
    let second = user_page(2);
    let absolute = format!("{ORIGIN_TOKEN}{second}");
    let server = RecordingServer::start(vec![
        ScriptedResponse::new(200)
            .with_header("Link", &link_header(&absolute, LinkRelation::Next))
            .with_body(bare_page(&[repository_json(Repository {
                id: 1,
                owner: "acme",
                name: "one",
            })])),
        ScriptedResponse::new(200).with_body(bare_page(&[repository_json(Repository {
            id: 2,
            owner: "acme",
            name: "two",
        })])),
    ])
    .expect("the scripted server should start");
    let client = user_client(&server.origin(), ListingLimits::defaults(), TEST_TIMEOUT)
        .expect("a user-mode adapter should construct");

    let repositories = client
        .list_repositories()
        .expect("an absolute continuation on the listing origin is followed");

    assert_eq!(
        repositories
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>(),
        vec!["acme/one".to_owned(), "acme/two".to_owned()],
        "pages must be appended in request order"
    );
    let requests = server.requests();
    assert_eq!(requests.len(), 2, "both pages must be requested");
    assert_eq!(
        requests.get(1).map(|request| request.target.as_str()),
        Some(second.as_str()),
        "the continuation must request the reported page"
    );
}

/// The ways a first page can present the continuation it names.
#[derive(Clone, Copy, Debug)]
enum ContinuationPresentation {
    /// One relative `next` link.
    Relative,
    /// A relation list naming `next` alongside `last`.
    RelationList,
    /// A `last` link in a header that precedes the one naming `next`.
    SeparateHeaders,
    /// A next link with an extra quoted parameter holding a comma and a quote.
    QuotedParameter,
}

impl ContinuationPresentation {
    /// Returns the `Link` header values this presentation sends.
    fn values(self, target: &str) -> Vec<String> {
        match self {
            Self::Relative => vec![link_header(target, LinkRelation::Next)],
            Self::RelationList => vec![link_header(target, LinkRelation::NextThenLast)],
            Self::SeparateHeaders => vec![
                link_header(target, LinkRelation::Last),
                link_header(target, LinkRelation::Next),
            ],
            Self::QuotedParameter => vec![format!(
                "{link}; title=\"a,b\\\"c\"",
                link = link_header(target, LinkRelation::Next)
            )],
        }
    }
}

#[rstest]
#[case::relative(ContinuationPresentation::Relative)]
#[case::relation_list(ContinuationPresentation::RelationList)]
#[case::separate_headers(ContinuationPresentation::SeparateHeaders)]
#[case::quoted_parameter(ContinuationPresentation::QuotedParameter)]
fn a_next_link_is_followed_however_the_header_presents_it(
    #[case] presentation: ContinuationPresentation,
) {
    let second = user_page(2);
    let first_page = presentation
        .values(&second)
        .iter()
        .fold(ScriptedResponse::new(200), |response, value| {
            response.with_header("Link", value)
        });
    let server = RecordingServer::start(vec![
        first_page.with_body(bare_page(&[repository_json(Repository {
            id: 1,
            owner: "acme",
            name: "one",
        })])),
        ScriptedResponse::new(200).with_body(bare_page(&[repository_json(Repository {
            id: 2,
            owner: "acme",
            name: "two",
        })])),
    ])
    .expect("the scripted server should start");
    let client = user_client(&server.origin(), ListingLimits::defaults(), TEST_TIMEOUT)
        .expect("a user-mode adapter should construct");

    let repositories = client
        .list_repositories()
        .expect("a continuation stays on the listing it was reported for");

    assert_eq!(
        repositories
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>(),
        vec!["acme/one".to_owned(), "acme/two".to_owned()],
        "both pages must be observed in order for {presentation:?}"
    );
    let requests = server.requests();
    assert_eq!(
        requests.get(1).map(|request| request.target.as_str()),
        Some(second.as_str()),
        "the continuation must request the reported page for {presentation:?}"
    );
}

#[test]
fn unknown_fields_and_a_null_default_branch_are_accepted() {
    let record = concat!(
        r#"{"id":3,"name":"legacy","private":true,"default_branch":null,"#,
        r#""owner":{"login":"acme","extra":"ignored"},"archived":false}"#
    );
    let server = RecordingServer::start(vec![
        ScriptedResponse::new(200).with_body(format!("[{record}]")),
    ])
    .expect("the scripted server should start");
    let client = user_client(&server.origin(), ListingLimits::defaults(), TEST_TIMEOUT)
        .expect("a user-mode adapter should construct");

    let repositories = client
        .list_repositories()
        .expect("unknown fields and a null default branch are ordinary");

    let repository = repositories
        .first()
        .expect("the only record should be returned");

    assert_eq!(
        repository.default_branch(),
        None,
        "a repository with no commits has no default branch"
    );
    assert!(
        repository.is_private(),
        "privacy must be carried through from the wire record"
    );
}

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
        logs.contains("pages=1"),
        "the completion event must carry the page count: {logs}"
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
