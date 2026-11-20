//! Generated traversal tests for the repository listing adapter.
//!
//! The property under test is prefix preservation with terminal
//! correspondence: for any page partition the server reports, the adapter
//! returns exactly the concatenation of the pages in request order, and the
//! traversal ends where the `Link` headers say it ends. Both shipped profiles
//! are generated over, because each decodes its pages through a different body
//! shape. The fixtures are built independently of the adapter, so a traversal
//! that stops early, appends out of order, or loses a page fails here rather
//! than in a review.
//!
//! These are sampled tests over explicit finite ranges, not an exhaustive
//! proof. The named examples alongside the generated property are witnesses
//! for the classes the generator reaches rarely: empty results, single pages,
//! repeated identifiers, empty middle pages, and a terminal page of exactly
//! one hundred records.

mod common;

use std::time::Duration;

use common::{installation_client::installation_client, user_client::user_client};
use pretty_assertions::assert_eq;
use proptest::prelude::*;
use repovec_core::repository_listing::{
    RepositoryId,
    RepositoryListing,
    RepositoryListingError,
    RepositoryName,
    RepositoryOwner,
    RepositorySummary,
};
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

/// The request deadline the generated tests allow themselves.
const TEST_TIMEOUT: Duration = Duration::from_secs(5);

/// The largest page GitHub returns, and so the largest page generated.
const FULL_PAGE: usize = 100;

/// One generated record: an identifier, an owner login, and a name.
#[derive(Clone, Copy, Debug)]
struct Record {
    /// Numeric identifier, drawn from a small range so repeats occur.
    id: u64,
    /// Owner login the record belongs to.
    owner: &'static str,
    /// Repository name within that owner.
    name: &'static str,
}

/// Which credential kind a generated history is served to.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
enum Principal {
    /// A user credential, whose pages are bare arrays.
    #[default]
    User,
    /// An installation credential, whose pages are enveloped.
    Installation,
}

/// Owner logins the generator chooses between.
const OWNERS: &[&str] = &["acme", "octocat", "beta"];

/// Repository names the generator chooses between.
const NAMES: &[&str] = &["alpha", "bravo", "charlie", "delta"];

/// Generates one record with a small identifier space, so repeats are common.
fn record() -> impl Strategy<Value = Record> {
    (
        1..=6_u64,
        prop::sample::select(OWNERS.to_vec()),
        prop::sample::select(NAMES.to_vec()),
    )
        .prop_map(|(id, owner, name)| Record { id, owner, name })
}

/// Generates between one and thirty-two pages of zero to one hundred records.
///
/// The plan's generated range is 1–32 pages, so the terminal page is reached
/// after as many continuations as a small real listing would need.
fn page_histories() -> impl Strategy<Value = Vec<Vec<Record>>> {
    prop::collection::vec(prop::collection::vec(record(), 0..=FULL_PAGE), 1..=32)
}

/// Generates both shipped profiles alongside a page history.
fn served_histories() -> impl Strategy<Value = (Principal, Vec<Vec<Record>>)> {
    (any::<bool>(), page_histories()).prop_map(|(is_installation, pages)| {
        let principal = if is_installation {
            Principal::Installation
        } else {
            Principal::User
        };
        (principal, pages)
    })
}

/// Builds the JSON body the server returns for one page.
fn body_of(principal: Principal, page: &[Record]) -> String {
    let items = page
        .iter()
        .map(|record| {
            repository_json(Repository {
                id: record.id,
                owner: record.owner,
                name: record.name,
            })
        })
        .collect::<Vec<_>>();
    match principal {
        Principal::User => bare_page(&items),
        Principal::Installation => envelope_page(&items),
    }
}

/// Builds the request target of the page numbered `number`.
fn target(principal: Principal, number: u32) -> String {
    match principal {
        Principal::User => user_page(number),
        Principal::Installation => installation_page(number),
    }
}

/// Builds the scripted responses that serve `pages` in order.
fn script_of(principal: Principal, pages: &[Vec<Record>]) -> Vec<ScriptedResponse> {
    pages
        .iter()
        .enumerate()
        .map(|(index, page)| {
            let is_terminal = index + 1 == pages.len();
            let page_number = u32::try_from(index + 2).unwrap_or(u32::MAX);
            let response = ScriptedResponse::new(200).with_body(body_of(principal, page));
            if is_terminal {
                response
            } else {
                let next = target(principal, page_number);
                response.with_header("Link", &link_header(&next, LinkRelation::Next))
            }
        })
        .collect()
}

/// Builds the domain records the pages describe, in page order.
///
/// # Errors
///
/// Returns the domain's rejection when a generated value is not acceptable.
/// The generator draws only values the domain accepts, so a rejection means
/// the fixture and the domain disagree.
fn expected(pages: &[Vec<Record>]) -> Result<Vec<RepositorySummary>, RepositoryListingError> {
    pages
        .iter()
        .flatten()
        .map(|record| {
            let id = RepositoryId::new(record.id)?;
            let owner = RepositoryOwner::new(record.owner)?;
            let name = RepositoryName::new(record.name)?;
            Ok(
                RepositorySummary::new(id, owner, name)
                    .with_details(false, Some("main".to_owned())),
            )
        })
        .collect()
}

/// Runs the real adapter over the scripted pages and returns its result.
fn list(principal: Principal, pages: &[Vec<Record>]) -> Result<Vec<RepositorySummary>, String> {
    let server = RecordingServer::start(script_of(principal, pages))
        .map_err(|error| format!("the scripted server should start: {error}"))?;
    let client = match principal {
        Principal::User => user_client(&server.origin(), ListingLimits::defaults(), TEST_TIMEOUT),
        Principal::Installation => {
            installation_client(&server.origin(), ListingLimits::defaults(), TEST_TIMEOUT)
        }
    }
    .map_err(|error| format!("an adapter should construct: {error}"))?;
    let result = client.list_repositories();
    let observed = server.request_count();
    if observed != pages.len() {
        return Err(format!(
            "the traversal made {observed} requests for {} pages",
            pages.len()
        ));
    }
    result.map_err(|error| format!("the traversal failed: {error}"))
}

/// Builds `count` records with distinct identifiers.
fn records(count: usize) -> Vec<Record> {
    let owners = OWNERS.iter().copied().cycle();
    let names = NAMES.iter().copied().cycle();
    (1_u64..)
        .take(count)
        .zip(owners)
        .zip(names)
        .map(|((id, owner), name)| Record { id, owner, name })
        .collect()
}

proptest! {
    #![proptest_config(ProptestConfig { cases: 64, ..ProptestConfig::default() })]

    /// Every page partition is returned whole, in request order.
    #[test]
    fn a_listing_returns_every_page_in_request_order(
        (principal, pages) in served_histories(),
    ) {
        let actual = list(principal, &pages);
        let Ok(wanted) = expected(&pages) else {
            return Err(TestCaseError::fail("a generated value was outside the domain"));
        };
        prop_assert_eq!(actual.as_deref(), Ok(wanted.as_slice()));
    }
}

#[test]
fn an_empty_listing_is_a_single_empty_page() {
    let pages = vec![Vec::new()];

    let repositories =
        list(Principal::User, &pages).expect("an empty terminal page is a successful listing");

    assert_eq!(
        repositories,
        Vec::new(),
        "an account with no repositories lists nothing"
    );
}

#[test]
fn a_single_enveloped_page_is_returned_whole() {
    let pages = vec![vec![Record {
        id: 1,
        owner: "acme",
        name: "bravo",
    }]];

    let repositories =
        list(Principal::Installation, &pages).expect("a single terminal page should complete");

    assert_eq!(
        repositories,
        expected(&pages).expect("the fixture values must satisfy the domain"),
        "the only page must be returned unchanged"
    );
}

#[test]
fn repeated_identifiers_are_all_returned() {
    let repeated = Record {
        id: 4,
        owner: "beta",
        name: "delta",
    };
    let pages = vec![vec![repeated, repeated], vec![repeated]];

    let repositories =
        list(Principal::User, &pages).expect("repeated identifiers are not a failure");

    assert_eq!(
        repositories.len(),
        3,
        "the traversal must not deduplicate observations"
    );
}

#[test]
fn an_empty_middle_page_does_not_end_the_listing() {
    let pages = vec![
        vec![Record {
            id: 1,
            owner: "acme",
            name: "alpha",
        }],
        Vec::new(),
        vec![Record {
            id: 2,
            owner: "octocat",
            name: "bravo",
        }],
    ];

    let repositories =
        list(Principal::User, &pages).expect("an empty middle page still has a continuation");

    assert_eq!(
        repositories,
        expected(&pages).expect("the fixture values must satisfy the domain"),
        "completion is decided by the absence of a next link, not by page size"
    );
}

#[test]
fn a_terminal_page_of_exactly_one_hundred_records_is_accepted() {
    let pages = vec![records(FULL_PAGE)];

    let repositories =
        list(Principal::User, &pages).expect("a full terminal page is an ordinary page");

    assert_eq!(
        repositories.len(),
        FULL_PAGE,
        "a full page without a next link completes the listing"
    );
}

#[test]
fn a_full_page_followed_by_an_empty_terminal_page_is_accepted() {
    let pages = vec![records(FULL_PAGE), Vec::new()];

    let repositories =
        list(Principal::Installation, &pages).expect("the continuation decides the end");

    assert_eq!(
        repositories.len(),
        FULL_PAGE,
        "a full page must not be assumed terminal"
    );
}
