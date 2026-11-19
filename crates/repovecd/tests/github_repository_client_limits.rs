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
    repository_listing_wire::{bare_page, link_header, repository_json, user_page},
};
use repovecd::github_repository_client::ListingLimits;
use rstest::rstest;

/// The request deadline the budget tests allow themselves.
const TEST_TIMEOUT: Duration = Duration::from_secs(5);

/// Builds a page body holding `count` records.
fn page_of(count: usize) -> String {
    let items = (1_u64..)
        .take(count)
        .map(|id| repository_json(id, "acme", "repository"))
        .collect::<Vec<_>>();
    bare_page(&items)
}

/// Builds a page whose `Link` header names the next page.
fn continuation_page(count: usize, next: u32) -> ScriptedResponse {
    ScriptedResponse::new(200)
        .with_header("Link", &link_header(&user_page(next), "next"))
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

#[test]
fn a_page_exactly_at_the_record_budget_is_accepted() {
    let limits = ListingLimits::defaults().with_max_records(3);
    let server = RecordingServer::start(vec![ScriptedResponse::new(200).with_body(page_of(3))])
        .expect("the scripted server should start");
    let client = user_client(&server.origin(), limits, TEST_TIMEOUT)
        .expect("a user-mode adapter should construct");

    let repositories = client
        .list_repositories()
        .expect("a terminal page exactly at the budget is valid");

    assert_eq!(
        repositories.len(),
        3,
        "the boundary page must be returned whole"
    );
}

#[test]
fn a_page_one_record_past_the_budget_fails_without_a_listing() {
    let limits = ListingLimits::defaults().with_max_records(2);
    let server = RecordingServer::start(vec![ScriptedResponse::new(200).with_body(page_of(3))])
        .expect("the scripted server should start");
    let client = user_client(&server.origin(), limits, TEST_TIMEOUT)
        .expect("a user-mode adapter should construct");

    let error = expect_failure(client.list_repositories());

    assert_eq!(
        error,
        RepositoryListingError::ResourceLimitExceeded {
            budget: ResourceBudget::Records,
            limit: 2,
        },
        "the record budget must be named when it is crossed"
    );
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
fn a_page_budget_of_one_admits_a_single_terminal_page() {
    let limits = ListingLimits::defaults().with_max_pages(1);
    let server = RecordingServer::start(vec![ScriptedResponse::new(200).with_body(page_of(1))])
        .expect("the scripted server should start");
    let client = user_client(&server.origin(), limits, TEST_TIMEOUT)
        .expect("a user-mode adapter should construct");

    let repositories = client
        .list_repositories()
        .expect("one terminal page fits a one-page budget");

    assert_eq!(repositories.len(), 1, "the single page must be returned");
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
fn a_body_exactly_at_the_page_byte_budget_is_accepted() {
    let body = page_of(1);
    let budget = u64::try_from(body.len()).expect("a test body length fits in u64");
    let limits = ListingLimits::defaults().with_max_page_bytes(budget);
    let server = RecordingServer::start(vec![ScriptedResponse::new(200).with_body(body)])
        .expect("the scripted server should start");
    let client = user_client(&server.origin(), limits, TEST_TIMEOUT)
        .expect("a user-mode adapter should construct");

    let repositories = client
        .list_repositories()
        .expect("a body exactly at the byte budget is accepted");

    assert_eq!(repositories.len(), 1, "the boundary body must decode");
}

#[test]
fn a_body_one_byte_past_the_page_byte_budget_fails() {
    let body = page_of(1);
    let budget = u64::try_from(body.len() - 1).expect("a test body length fits in u64");
    let limits = ListingLimits::defaults().with_max_page_bytes(budget);
    let server = RecordingServer::start(vec![ScriptedResponse::new(200).with_body(body)])
        .expect("the scripted server should start");
    let client = user_client(&server.origin(), limits, TEST_TIMEOUT)
        .expect("a user-mode adapter should construct");

    let error = expect_failure(client.list_repositories());

    assert_eq!(
        error,
        RepositoryListingError::ResourceLimitExceeded {
            budget: ResourceBudget::PageBytes,
            limit: budget,
        },
        "an oversized body must be rejected before it is decoded"
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
            .with_header("Link", &link_header(&user_page(2), "next"))
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
        repository_json(1, "acme", "one"),
        repository_json(2, "acme", "two")
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
        repository_json(1, "acme", "one")
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
    let header = link_header(&format!("{ORIGIN_TOKEN}{}", user_page(2)), "next");
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
