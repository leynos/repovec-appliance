//! Budget tests for the GitHub repository listing adapter.
//!
//! Each budget is exercised at its exact boundary, where a terminal response
//! must still succeed, and one step beyond it, where the traversal must fail
//! with the exhausted budget named. A budget that is crossed after earlier
//! pages were accepted still fails the whole listing.

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
        link_header,
        repository_json,
        user_page,
    },
};
use repovecd::github_repository_client::ListingLimits;
use rstest::rstest;

/// The request deadline the budget tests allow themselves.
const TEST_TIMEOUT: Duration = Duration::from_secs(5);

/// Builds a page body holding `count` records.
fn page_of(count: usize) -> String {
    let items = (1_u64..)
        .take(count)
        .map(|id| {
            repository_json(Repository {
                id,
                owner: "acme",
                name: "repository",
            })
        })
        .collect::<Vec<_>>();
    bare_page(&items)
}

/// Builds a page whose `Link` header names the next page.
fn continuation_page(count: usize, next: u32) -> ScriptedResponse {
    ScriptedResponse::new(200)
        .with_header("Link", &link_header(&user_page(next), LinkRelation::Next))
        .with_body(page_of(count))
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

/// A budget boundary the adapter must decide exactly where it is crossed.
#[derive(Clone, Copy, Debug)]
enum BudgetProbe {
    /// A terminal page exactly at the record budget.
    RecordsAccepted,
    /// A terminal page holding one record past the record budget.
    RecordsRefused,
    /// A single terminal page that fills a one-page budget.
    PagesAccepted,
    /// A body exactly at the page-byte budget.
    PageBytesAccepted,
    /// A body one byte past the page-byte budget.
    PageBytesRefused,
}

/// The outcome a budget probe expects from its listing.
#[derive(Clone, Copy, Debug)]
enum Expectation {
    /// The listing completes with this many records.
    Records(usize),
    /// The listing fails naming this exhausted budget and its limit.
    Exhausted(ResourceBudget, u64),
}

impl BudgetProbe {
    /// Returns the body the probe serves.
    fn body(self) -> String {
        match self {
            Self::RecordsAccepted | Self::RecordsRefused => page_of(3),
            Self::PagesAccepted | Self::PageBytesAccepted | Self::PageBytesRefused => page_of(1),
        }
    }

    /// Returns the size of the served body in bytes.
    fn body_bytes(self) -> u64 {
        // A test body is far smaller than `u64::MAX`, so saturation is only a
        // total-function guard rather than an expected outcome.
        u64::try_from(self.body().len()).unwrap_or(u64::MAX)
    }

    /// Returns the limits the probe configures.
    fn limits(self) -> ListingLimits {
        match self {
            Self::RecordsAccepted => ListingLimits::defaults().with_max_records(3),
            Self::RecordsRefused => ListingLimits::defaults().with_max_records(2),
            Self::PagesAccepted => ListingLimits::defaults().with_max_pages(1),
            Self::PageBytesAccepted => {
                ListingLimits::defaults().with_max_page_bytes(self.body_bytes())
            }
            Self::PageBytesRefused => {
                ListingLimits::defaults().with_max_page_bytes(self.body_bytes().saturating_sub(1))
            }
        }
    }

    /// Returns the outcome the probe expects.
    fn expected(self) -> Expectation {
        match self {
            Self::RecordsAccepted => Expectation::Records(3),
            Self::PagesAccepted | Self::PageBytesAccepted => Expectation::Records(1),
            Self::RecordsRefused => Expectation::Exhausted(ResourceBudget::Records, 2),
            Self::PageBytesRefused => Expectation::Exhausted(
                ResourceBudget::PageBytes,
                self.body_bytes().saturating_sub(1),
            ),
        }
    }
}

#[rstest]
#[case::records_at_the_budget(BudgetProbe::RecordsAccepted)]
#[case::records_past_the_budget(BudgetProbe::RecordsRefused)]
#[case::pages_at_the_budget(BudgetProbe::PagesAccepted)]
#[case::page_bytes_at_the_budget(BudgetProbe::PageBytesAccepted)]
#[case::page_bytes_past_the_budget(BudgetProbe::PageBytesRefused)]
fn a_budget_boundary_is_decided_where_it_is_crossed(#[case] probe: BudgetProbe) {
    let server = RecordingServer::start(vec![ScriptedResponse::new(200).with_body(probe.body())])
        .expect("the scripted server should start");
    let client = user_client(&server.origin(), probe.limits(), TEST_TIMEOUT)
        .expect("a user-mode adapter should construct");

    let result = client.list_repositories();

    match probe.expected() {
        Expectation::Records(count) => {
            let repositories = result.expect("a terminal page inside every budget is valid");
            assert_eq!(
                repositories.len(),
                count,
                "the boundary page must be returned whole for {probe:?}"
            );
        }
        Expectation::Exhausted(budget, limit) => {
            assert_eq!(
                expect_failure(result),
                RepositoryListingError::ResourceLimitExceeded { budget, limit },
                "the exhausted budget must be named for {probe:?}"
            );
        }
    }
}

#[test]
fn a_record_budget_crossed_on_a_later_page_fails_the_whole_listing() {
    let limits = ListingLimits::defaults().with_max_records(3);
    let server = RecordingServer::start(vec![
        continuation_page(2, 2),
        ScriptedResponse::new(200).with_body(page_of(2)),
    ])
    .expect("the scripted server should start");
    let client = user_client(&server.origin(), limits, TEST_TIMEOUT)
        .expect("a user-mode adapter should construct");

    let error = expect_failure(client.list_repositories());

    assert_eq!(
        error,
        RepositoryListingError::ResourceLimitExceeded {
            budget: ResourceBudget::Records,
            limit: 3,
        },
        "records accepted on earlier pages still count toward the budget"
    );
    assert_eq!(
        server.request_count(),
        2,
        "the crossing page is the last request made"
    );
}

#[test]
fn an_endless_continuation_is_bounded_by_the_page_budget() {
    let limits = ListingLimits::defaults().with_max_pages(3);
    let script = (1..=4)
        .map(|next| continuation_page(1, next))
        .collect::<Vec<_>>();
    let server = RecordingServer::start(script).expect("the scripted server should start");
    let client = user_client(&server.origin(), limits, TEST_TIMEOUT)
        .expect("a user-mode adapter should construct");

    let error = expect_failure(client.list_repositories());

    assert_eq!(
        error,
        RepositoryListingError::ResourceLimitExceeded {
            budget: ResourceBudget::Pages,
            limit: 3,
        },
        "a listing that never terminates must exhaust the page budget"
    );
    assert_eq!(
        server.request_count(),
        3,
        "the traversal must stop at the budget rather than follow every link"
    );
}

#[test]
fn the_cumulative_byte_budget_counts_every_accepted_page() {
    let body = page_of(1);
    let page_bytes = u64::try_from(body.len()).expect("a test body length fits in u64");
    let limits = ListingLimits::defaults()
        .with_max_page_bytes(page_bytes)
        .with_max_total_bytes(page_bytes);
    let server = RecordingServer::start(vec![
        ScriptedResponse::new(200)
            .with_header("Link", &link_header(&user_page(2), LinkRelation::Next))
            .with_body(body.clone()),
        ScriptedResponse::new(200).with_body(body),
    ])
    .expect("the scripted server should start");
    let client = user_client(&server.origin(), limits, TEST_TIMEOUT)
        .expect("a user-mode adapter should construct");

    let error = expect_failure(client.list_repositories());

    assert_eq!(
        error,
        RepositoryListingError::ResourceLimitExceeded {
            budget: ResourceBudget::TotalBytes,
            limit: page_bytes,
        },
        "two accepted pages must not share one page-sized aggregate budget"
    );
}

#[test]
fn total_count_never_admits_an_extra_record() {
    let limits = ListingLimits::defaults().with_max_records(1);
    let envelope = format!(
        r#"{{"total_count":99,"repositories":[{},{}]}}"#,
        repository_json(Repository {
            id: 1,
            owner: "acme",
            name: "one"
        }),
        repository_json(Repository {
            id: 2,
            owner: "acme",
            name: "two"
        })
    );
    let server = RecordingServer::start(vec![ScriptedResponse::new(200).with_body(envelope)])
        .expect("the scripted server should start");
    let client = installation_client(&server.origin(), limits, TEST_TIMEOUT)
        .expect("an installation-mode adapter should construct");

    let error = expect_failure(client.list_repositories());

    assert_eq!(
        error,
        RepositoryListingError::ResourceLimitExceeded {
            budget: ResourceBudget::Records,
            limit: 1,
        },
        "the budget counts the items returned, not the reported total"
    );
}

#[test]
fn a_reported_total_larger_than_the_items_is_only_informational() {
    let limits = ListingLimits::defaults().with_max_records(1);
    let envelope = format!(
        r#"{{"total_count":99,"repositories":[{}]}}"#,
        repository_json(Repository {
            id: 1,
            owner: "acme",
            name: "one"
        })
    );
    let server = RecordingServer::start(vec![ScriptedResponse::new(200).with_body(envelope)])
        .expect("the scripted server should start");
    let client = installation_client(&server.origin(), limits, TEST_TIMEOUT)
        .expect("an installation-mode adapter should construct");

    let repositories = client
        .list_repositories()
        .expect("a reported total is not a termination condition");

    assert_eq!(
        repositories.len(),
        1,
        "the single returned item is the whole listing"
    );
}

#[rstest]
#[case::page_bytes(
    ListingLimits::defaults().with_max_page_bytes(1),
    ResourceBudget::PageBytes
)]
#[case::total_bytes(
    ListingLimits::defaults().with_max_total_bytes(1),
    ResourceBudget::TotalBytes
)]
#[case::link_header_bytes(
    ListingLimits::defaults().with_max_link_header_bytes(1),
    ResourceBudget::LinkHeaderBytes
)]
fn an_unretainable_page_is_not_followed(
    #[case] limits: ListingLimits,
    #[case] budget: ResourceBudget,
) {
    let header = link_header(
        &format!("{ORIGIN_TOKEN}{}", user_page(2)),
        LinkRelation::Next,
    );
    let server = RecordingServer::start(vec![
        ScriptedResponse::new(200)
            .with_header("Link", &header)
            .with_body(page_of(1)),
        ScriptedResponse::new(200).with_body(page_of(1)),
    ])
    .expect("the scripted server should start");
    let client = user_client(&server.origin(), limits, TEST_TIMEOUT)
        .expect("a user-mode adapter should construct");

    let error = expect_failure(client.list_repositories());

    assert_eq!(
        error,
        RepositoryListingError::ResourceLimitExceeded { budget, limit: 1 },
        "the exhausted budget must be named"
    );
    assert_eq!(
        server.request_count(),
        1,
        "a page that cannot be accepted must not be followed"
    );
}

#[test]
fn an_identity_over_its_budget_stops_before_the_first_request() {
    let limits = ListingLimits::defaults().with_max_continuation_identity_bytes(1);
    let server = RecordingServer::start(vec![ScriptedResponse::new(200).with_body(page_of(1))])
        .expect("the scripted server should start");
    let client = user_client(&server.origin(), limits, TEST_TIMEOUT)
        .expect("a user-mode adapter should construct");

    let error = expect_failure(client.list_repositories());

    assert_eq!(
        error,
        RepositoryListingError::ResourceLimitExceeded {
            budget: ResourceBudget::ContinuationIdentityBytes,
            limit: 1,
        },
        "the initial request's identity is subject to the same budget"
    );
    assert_eq!(
        server.request_count(),
        0,
        "an unretainable request identity must be refused before it is sent"
    );
}
