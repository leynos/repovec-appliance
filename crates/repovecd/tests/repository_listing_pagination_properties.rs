//! Generated pagination-progress tests for the listing traversal.
//!
//! The property under test is progress: a generated continuation history either
//! terminates at its terminal page within the page budget, or fails with the
//! typed budget error once the budget is reached, and a history that revisits
//! a request fails with the typed cycle error. Every generated next link keeps
//! the listing's fixed query pairs on the permitted path, so a traversal that
//! follows a changed filter is rejected rather than measured as progress. Both
//! shipped profiles are generated over, because each pages a different path
//! and decodes a different body shape.
//!
//! The generated cases assert exact request counts as well as the typed
//! outcome, because "finite" is only meaningful with the bound the budget
//! states. A traversal that keeps requesting pages after a cycle, or that
//! stops short of the budget it was given, fails here rather than in review.
//! These are sampled tests over explicit finite ranges, not proofs.

mod common;

use std::time::Duration;

use common::{installation_client::installation_client, user_client::user_client};
use pretty_assertions::assert_eq;
use proptest::prelude::*;
use repovec_core::repository_listing::{RepositoryListing, RepositoryListingError, ResourceBudget};
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
use repovecd::github_repository_client::{GitHubRepositoryClient, ListingLimits};

/// The request deadline these generated tests allow themselves.
const TEST_TIMEOUT: Duration = Duration::from_secs(5);

/// Which credential kind a generated history is served to.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Principal {
    /// A user credential, whose pages are bare arrays.
    User,
    /// An installation credential, whose pages are enveloped.
    Installation,
}

impl Principal {
    /// Chooses the principal a generated flag names.
    const fn from_flag(is_installation: bool) -> Self {
        if is_installation {
            Self::Installation
        } else {
            Self::User
        }
    }

    /// Builds the one-record body of the page numbered `page`.
    fn body(self, page: u32) -> String {
        let items = [repository_json(Repository {
            id: u64::from(page),
            owner: "acme",
            name: "paged",
        })];
        match self {
            Self::User => bare_page(&items),
            Self::Installation => envelope_page(&items),
        }
    }

    /// Builds the request target of the page numbered `page`.
    fn target(self, page: u32) -> String {
        match self {
            Self::User => user_page(page),
            Self::Installation => installation_page(page),
        }
    }

    /// Builds an adapter for this principal at a loopback origin.
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

/// Builds a page numbered `page` whose next link names page `next`.
fn page_with_next(principal: Principal, page: u32, next: u32) -> ScriptedResponse {
    let header = link_header(&principal.target(next), LinkRelation::Next);
    ScriptedResponse::new(200)
        .with_header("Link", &header)
        .with_body(principal.body(page))
}

/// Builds the terminal page numbered `page`.
fn terminal_page(principal: Principal, page: u32) -> ScriptedResponse {
    ScriptedResponse::new(200).with_body(principal.body(page))
}

/// Scripts `pages` pages that each continue to the next, ending terminal.
fn fresh_chain(principal: Principal, pages: u32) -> Vec<ScriptedResponse> {
    (1..=pages)
        .map(|page| {
            if page == pages {
                terminal_page(principal, page)
            } else {
                page_with_next(principal, page, page + 1)
            }
        })
        .collect()
}

/// Scripts `pages` pages whose last link returns to page two.
fn cyclic_chain(principal: Principal, pages: u32) -> Vec<ScriptedResponse> {
    let mut script = (1..pages)
        .map(|page| page_with_next(principal, page, page + 1))
        .collect::<Vec<_>>();
    script.push(page_with_next(principal, pages, 2));
    script
}

proptest! {
    #![proptest_config(ProptestConfig { cases: 64, ..ProptestConfig::default() })]

    /// A fresh history completes or stops exactly at the page budget.
    #[test]
    fn a_generated_fresh_history_completes_or_reaches_the_page_budget(
        is_installation in any::<bool>(),
        budget in 1_u64..=6,
        chain in 1_u32..=8,
    ) {
        let principal = Principal::from_flag(is_installation);
        let server = RecordingServer::start(fresh_chain(principal, chain))
            .expect("the scripted server should start");
        let client = principal
            .client(&server.origin(), ListingLimits::defaults().with_max_pages(budget))
            .expect("an adapter should construct");

        let result = client.list_repositories();
        let requests = server.request_count();

        if u64::from(chain) <= budget {
            let repositories = result.expect("a fresh history within budget must complete");
            prop_assert_eq!(
                repositories.len(),
                usize::try_from(chain).unwrap_or(usize::MAX),
                "every generated page must contribute its record for {:?}",
                principal
            );
            prop_assert_eq!(
                requests,
                usize::try_from(chain).unwrap_or(usize::MAX),
                "a complete history must request each page once for {:?}",
                principal
            );
        } else {
            prop_assert_eq!(
                result.err(),
                Some(RepositoryListingError::ResourceLimitExceeded {
                    budget: ResourceBudget::Pages,
                    limit: budget,
                }),
                "a history past the budget must fail with the pages budget for {:?}",
                principal
            );
            prop_assert_eq!(
                requests,
                usize::try_from(budget).unwrap_or(usize::MAX),
                "the request past the budget must not be sent for {:?}",
                principal
            );
        }
    }

    /// A history that revisits a request fails as a cycle, finitely.
    #[test]
    fn a_generated_cycle_fails_after_a_finite_number_of_requests(
        is_installation in any::<bool>(),
        cycle_pages in 2_u32..=6,
    ) {
        let principal = Principal::from_flag(is_installation);
        let server = RecordingServer::start(cyclic_chain(principal, cycle_pages))
            .expect("the scripted server should start");
        let client = principal
            .client(&server.origin(), ListingLimits::defaults())
            .expect("an adapter should construct");

        let result = client.list_repositories();

        prop_assert_eq!(
            result.err(),
            Some(RepositoryListingError::CycleDetected),
            "a repeated request must be reported as a cycle for {:?}",
            principal
        );
        prop_assert_eq!(
            server.request_count(),
            usize::try_from(cycle_pages).unwrap_or(usize::MAX),
            "the repeated page must be recognized rather than requested again for {:?}",
            principal
        );
    }
}

#[test]
fn a_single_page_history_makes_one_request() {
    let principal = Principal::User;
    let server = RecordingServer::start(fresh_chain(principal, 1))
        .expect("the scripted server should start");
    let client = principal
        .client(&server.origin(), ListingLimits::defaults())
        .expect("a user-mode adapter should construct");

    let repositories = client
        .list_repositories()
        .expect("a terminal first page is a complete listing");

    assert_eq!(
        repositories.len(),
        1,
        "the terminal page's record must be returned"
    );
    assert_eq!(
        server.request_count(),
        1,
        "a terminal first page must not be followed by another request"
    );
}
