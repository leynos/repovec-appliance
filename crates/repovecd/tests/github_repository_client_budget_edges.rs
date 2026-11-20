//! Budget-edge tests for the GitHub repository listing adapter.
//!
//! A budget must admit a listing that lands exactly on its limit and refuse one
//! that crosses it, including when the crossing happens after earlier pages
//! were accepted. These cases cover the aggregate and `Link`-header budgets,
//! which the terminal-boundary matrix in `github_repository_client_limits.rs`
//! does not reach, over both shipped credential profiles.

mod common;

use std::time::Duration;

use common::{installation_client::installation_client, user_client::user_client};
use pretty_assertions::assert_eq;
use repovec_core::repository_listing::{RepositoryListing, RepositoryListingError, ResourceBudget};
use repovec_test_helpers::{
    recording_http::{ORIGIN_TOKEN, RecordingServer, ScriptedResponse},
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
use repovecd::github_repository_client::{GitHubRepositoryClient, ListingLimits};
use rstest::rstest;

/// The request deadline the budget-edge tests allow themselves.
const TEST_TIMEOUT: Duration = Duration::from_secs(5);

/// Which credential kind an adapter under test presents.
///
/// The budgets are enforced by the shared traversal, so each scenario runs over
/// both profiles: the profile decides only the page representation and path.
#[derive(Clone, Copy, Debug)]
enum Profile {
    /// A user credential, whose pages are bare arrays.
    User,
    /// An installation credential, whose pages are enveloped.
    Installation,
}

impl Profile {
    /// Wraps a page's records in the representation this profile expects.
    fn page(self, count: usize) -> String {
        let items = records(count);
        match self {
            Self::User => bare_page(&items),
            Self::Installation => envelope_page(&items),
        }
    }

    /// Returns the target of the page this profile's continuation names.
    fn next_page(self, page: u32) -> String {
        match self {
            Self::User => user_page(page),
            Self::Installation => installation_page(page),
        }
    }

    /// Builds an adapter for this profile at a loopback origin.
    fn client(
        self,
        origin: &str,
        limits: ListingLimits,
    ) -> Result<GitHubRepositoryClient, RepositoryListingError> {
        match self {
            Self::User => user_client(origin, limits, TEST_TIMEOUT),
            Self::Installation => installation_client(origin, limits, TEST_TIMEOUT),
        }
    }
}

/// Returns a page body's length, the unit the byte budgets are configured in.
fn page_bytes(text: &str) -> u64 { u64::try_from(text.len()).unwrap_or(u64::MAX) }

/// Builds one page's records.
fn records(count: usize) -> Vec<String> {
    (1_u64..)
        .take(count)
        .map(|id| {
            repository_json(Repository {
                id,
                owner: "acme",
                name: "repository",
            })
        })
        .collect()
}

/// Returns a `Link` header value's length, which its budget is configured in.
const fn header_bytes(text: &str) -> usize { text.len() }

/// Returns a `Link`-header limit as the domain error reports it.
fn reported_header_limit(text: &str) -> u64 {
    u64::try_from(header_bytes(text)).unwrap_or(u64::MAX)
}

/// Returns the error a listing produced, or panics with its unexpected success.
fn expect_failure(
    result: Result<Vec<impl std::fmt::Debug>, RepositoryListingError>,
) -> RepositoryListingError {
    match result {
        Ok(repositories) => panic!("the listing should have failed, got {repositories:?}"),
        Err(error) => error,
    }
}

#[rstest]
#[case::user(Profile::User)]
#[case::installation(Profile::Installation)]
fn a_listing_exactly_at_the_cumulative_byte_budget_succeeds(#[case] profile: Profile) {
    let body = profile.page(1);
    let per_page = page_bytes(&body);
    let limits = ListingLimits::defaults()
        .with_max_page_bytes(per_page)
        .with_max_total_bytes(2 * per_page);
    let server = RecordingServer::start(vec![
        ScriptedResponse::new(200)
            .with_header(
                "Link",
                &link_header(&profile.next_page(2), LinkRelation::Next),
            )
            .with_body(body.clone()),
        ScriptedResponse::new(200).with_body(body),
    ])
    .expect("the scripted server should start");
    let client = profile
        .client(&server.origin(), limits)
        .expect("an adapter for the named profile should construct");

    let repositories = client
        .list_repositories()
        .expect("two pages landing exactly on the aggregate budget are valid");

    assert_eq!(
        repositories.len(),
        2,
        "the aggregate budget admits the second page at its exact limit ({profile:?})"
    );
    assert_eq!(
        server.request_count(),
        2,
        "both pages must be requested exactly once ({profile:?})"
    );
}

#[rstest]
#[case::user(Profile::User)]
#[case::installation(Profile::Installation)]
fn a_link_header_exactly_at_its_budget_is_followed(#[case] profile: Profile) {
    let header = link_header(&profile.next_page(2), LinkRelation::Next);
    let limits = ListingLimits::defaults().with_max_link_header_bytes(header_bytes(&header));
    let server = RecordingServer::start(vec![
        ScriptedResponse::new(200)
            .with_header("Link", &header)
            .with_body(profile.page(1)),
        ScriptedResponse::new(200).with_body(profile.page(1)),
    ])
    .expect("the scripted server should start");
    let client = profile
        .client(&server.origin(), limits)
        .expect("an adapter for the named profile should construct");

    let repositories = client
        .list_repositories()
        .expect("a Link header exactly at its budget is retaintable");

    assert_eq!(
        repositories.len(),
        2,
        "the retaintable continuation must be followed to the terminal page ({profile:?})"
    );
    assert_eq!(
        server.request_count(),
        2,
        "the terminal page must be requested exactly once ({profile:?})"
    );
}

/// One later-page byte-budget scenario and the limit it crosses.
struct LaterByteBudget {
    /// The credential profile the traversal runs under.
    profile: Profile,
    /// The limits whose one crossed budget the scenario exercises.
    limits: ListingLimits,
    /// The budget the later page must exhaust.
    budget: ResourceBudget,
    /// The limit the domain error must report.
    limit: u64,
    /// Why this crossing must fail the whole listing, stated in the assertion.
    why: &'static str,
}

/// Builds a later-page scenario for one profile and one byte budget.
fn later_byte_budget(profile: Profile, budget: ResourceBudget) -> LaterByteBudget {
    let first_page_bytes = page_bytes(&profile.page(1));
    let header = link_header(&profile.next_page(2), LinkRelation::Next);
    match budget {
        ResourceBudget::LinkHeaderBytes => LaterByteBudget {
            profile,
            limits: ListingLimits::defaults().with_max_link_header_bytes(header_bytes(&header)),
            budget,
            limit: reported_header_limit(&header),
            why: "a Link header over its budget on a later page must fail the listing",
        },
        _ => LaterByteBudget {
            profile,
            limits: ListingLimits::defaults().with_max_page_bytes(first_page_bytes),
            budget: ResourceBudget::PageBytes,
            limit: first_page_bytes,
            why: "a body over its page budget on a later page must fail the listing",
        },
    }
}

#[rstest]
#[case::user_page_bytes(later_byte_budget(Profile::User, ResourceBudget::PageBytes))]
#[case::user_link_header_bytes(later_byte_budget(Profile::User, ResourceBudget::LinkHeaderBytes))]
#[case::installation_page_bytes(later_byte_budget(
    Profile::Installation,
    ResourceBudget::PageBytes
))]
#[case::installation_link_header_bytes(later_byte_budget(
    Profile::Installation,
    ResourceBudget::LinkHeaderBytes
))]
fn a_later_page_over_a_byte_budget_fails_the_whole_listing(#[case] case: LaterByteBudget) {
    let LaterByteBudget {
        profile,
        limits,
        budget,
        limit,
        why,
    } = case;
    // The first page stays inside every budget; the second crosses exactly one
    // of them, so the failure cannot be attributed to the accepted page.
    let server = RecordingServer::start(vec![
        ScriptedResponse::new(200)
            .with_header(
                "Link",
                &link_header(&profile.next_page(2), LinkRelation::Next),
            )
            .with_body(profile.page(1)),
        ScriptedResponse::new(200)
            .with_header(
                "Link",
                &link_header(
                    &format!("{ORIGIN_TOKEN}{}", profile.next_page(3)),
                    LinkRelation::Next,
                ),
            )
            .with_body(profile.page(3)),
    ])
    .expect("the scripted server should start");
    let client = profile
        .client(&server.origin(), limits)
        .expect("an adapter for the named profile should construct");

    let error = expect_failure(client.list_repositories());

    assert_eq!(
        error,
        RepositoryListingError::ResourceLimitExceeded { budget, limit },
        "{why} (profile {profile:?})"
    );
    assert_eq!(
        server.request_count(),
        2,
        "the crossing page must end the traversal without a further request ({profile:?})"
    );
}
