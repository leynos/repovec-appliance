//! Behavioural tests for discovering repositories visible to a principal.
//!
//! Each scenario scripts a loopback server and runs the real adapter against it,
//! so the behaviour under test is the one a caller sees: which repositories come
//! back, in what order, and what happens when a later page fails.

mod common;

use std::time::Duration;

use common::{installation_client::installation_client, user_client::user_client};
use repovec_core::repository_listing::{
    RepositoryListing,
    RepositoryListingError,
    RepositorySummary,
};
use repovec_test_helpers::{
    recording_http::{RecordingServer, ScriptedResponse},
    repository_listing_wire::{
        bare_page,
        envelope_page,
        installation_page,
        link_header,
        repository_json,
        user_page,
    },
};
use repovecd::github_repository_client::{GitHubRepositoryClient, ListingLimits};
use rstest::fixture;
use rstest_bdd_macros::{given, scenarios, then, when};

/// The request deadline the scenarios allow themselves.
const TEST_TIMEOUT: Duration = Duration::from_secs(5);

/// The HTTP status a scripted page answers with.
const OK: u16 = 200;

/// Which credential a scenario presents.
#[derive(Clone, Copy, Default, PartialEq, Eq)]
enum Principal {
    /// A user credential, whose pages are bare arrays.
    #[default]
    User,
    /// An installation credential, whose pages are enveloped.
    Installation,
}

/// One scripted page of a listing.
struct Page {
    /// Repository objects as GitHub would report them.
    records: Vec<String>,
    /// Whether the page carries a next link.
    has_next: bool,
    /// HTTP status the page answers with.
    status: u16,
}

impl Page {
    /// Creates a successful page holding the given records.
    const fn new(records: Vec<String>) -> Self {
        Self {
            records,
            has_next: false,
            status: OK,
        }
    }
}

/// The state one scenario observes.
#[derive(Default)]
struct ListingWorld {
    /// Credential kind the scenario presents.
    principal: Principal,
    /// Account login an installation scenario authenticates for.
    account: String,
    /// Scripted pages in request order.
    pages: Vec<Page>,
    /// Repository labels the scenario expects, in order.
    expected: Vec<String>,
    /// The listing result, once the scenario has run it.
    outcome: Option<Result<Vec<RepositorySummary>, RepositoryListingError>>,
    /// Whether the credential was ever accepted by the server.
    reached_server: bool,
}

/// Provides a fresh world to each scenario.
///
/// A scenario starts with no scripted pages and a user credential; its `given`
/// steps add whatever it needs.
#[fixture]
fn world() -> ListingWorld {
    let scripted_pages = Vec::new();
    ListingWorld {
        pages: scripted_pages,
        ..ListingWorld::default()
    }
}

impl ListingWorld {
    /// Appends a record to the numbered page and remembers its label.
    fn add_record(&mut self, page: usize, login: &str, name: &str) {
        let record = repository_json(1, login, name);
        while self.pages.len() <= page {
            self.pages.push(Page::new(Vec::new()));
        }
        if let Some(target) = self.pages.get_mut(page) {
            target.records.push(record);
        }
        self.expected.push(format!("{login}/{name}"));
    }
}

/// Builds the page body for the scenario's credential kind.
fn body(principal: Principal, records: &[String]) -> String {
    match principal {
        Principal::User => bare_page(records),
        Principal::Installation => envelope_page(records),
    }
}

/// Builds the request target of the page numbered `number`.
fn target(principal: Principal, number: u32) -> String {
    match principal {
        Principal::User => user_page(number),
        Principal::Installation => installation_page(number),
    }
}

/// Builds the scripted responses for the scenario's pages.
fn script(world: &ListingWorld) -> Vec<ScriptedResponse> {
    world
        .pages
        .iter()
        .enumerate()
        .map(|(index, page)| {
            let number = u32::try_from(index + 2).unwrap_or(u32::MAX);
            let response = if page.status == OK {
                ScriptedResponse::new(OK).with_body(body(world.principal, &page.records))
            } else {
                ScriptedResponse::new(page.status)
            };
            if page.has_next {
                let next = target(world.principal, number);
                response.with_header("Link", &link_header(&next, "next"))
            } else {
                response
            }
        })
        .collect()
}

/// Builds the adapter the scenario's credential kind requires.
fn client(world: &ListingWorld, origin: &str) -> Option<GitHubRepositoryClient> {
    match world.principal {
        Principal::User => user_client(origin, ListingLimits::defaults(), TEST_TIMEOUT).ok(),
        Principal::Installation => {
            installation_client(origin, ListingLimits::defaults(), TEST_TIMEOUT).ok()
        }
    }
}

/// Runs the listing and stores what the scenario observed.
fn run(world: &mut ListingWorld) {
    let Ok(server) = RecordingServer::start(script(world)) else {
        return;
    };
    let Some(adapter) = client(world, &server.origin()) else {
        return;
    };
    world.reached_server = true;
    world.outcome = Some(adapter.list_repositories());
}

#[given("a user credential accepted by the repository service")]
fn a_user_credential_accepted_by_the_repository_service(world: &mut ListingWorld) {
    world.principal = Principal::User;
}

#[given("a personal repository on the first page")]
fn a_personal_repository_on_the_first_page(world: &mut ListingWorld) {
    world.add_record(0, "octocat", "hello-world");
    if let Some(page) = world.pages.first_mut() {
        page.has_next = true;
    }
}

#[given("an organization repository on a later page")]
fn an_organization_repository_on_a_later_page(world: &mut ListingWorld) {
    world.add_record(1, "acme", "billing");
}

#[given("an installation credential for a {owner} account")]
fn an_installation_credential_for_an_account(world: &mut ListingWorld, owner: String) {
    world.principal = Principal::Installation;
    world.account = owner;
}

#[given("repositories for that account on two pages")]
fn repositories_for_that_account_on_two_pages(world: &mut ListingWorld) {
    let account = world.account.clone();
    world.add_record(0, &account, "repo-one");
    if let Some(page) = world.pages.first_mut() {
        page.has_next = true;
    }
    world.add_record(1, &account, "repo-two");
}

#[given("a successful first page containing a repository")]
fn a_successful_first_page_containing_a_repository(world: &mut ListingWorld) {
    world.add_record(0, "octocat", "hello-world");
    if let Some(page) = world.pages.first_mut() {
        page.has_next = true;
    }
}

#[given("the next page rejects the credential")]
fn the_next_page_rejects_the_credential(world: &mut ListingWorld) {
    world.pages.push(Page {
        records: Vec::new(),
        has_next: false,
        status: 401,
    });
}

#[given("an empty first page with a next link")]
fn an_empty_first_page_with_a_next_link(world: &mut ListingWorld) {
    world.pages.push(Page {
        records: Vec::new(),
        has_next: true,
        status: OK,
    });
}

#[given("a repository on the final page")]
fn a_repository_on_the_final_page(world: &mut ListingWorld) {
    world.add_record(1, "acme", "final");
}

#[when("repository listing completes")]
fn repository_listing_completes(world: &mut ListingWorld) { run(world); }

#[when("repository listing is attempted")]
fn repository_listing_is_attempted(world: &mut ListingWorld) { run(world); }

#[then("both repository observations are returned in page order")]
fn both_repository_observations_are_returned_in_page_order(world: &ListingWorld) {
    let Some(Ok(repositories)) = world.outcome.as_ref() else {
        panic!("the listing should have completed, got {:?}", world.outcome);
    };
    let observed = repositories
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>();
    assert_eq!(
        observed, world.expected,
        "observations must follow request order"
    );
}

#[then("both repository observations for that account are returned")]
fn both_repository_observations_for_that_account_are_returned(world: &ListingWorld) {
    let Some(Ok(repositories)) = world.outcome.as_ref() else {
        panic!("the listing should have completed, got {:?}", world.outcome);
    };
    assert_eq!(
        repositories.len(),
        2,
        "both pages must contribute one observation each"
    );
    for repository in repositories {
        assert_eq!(
            repository.owner().as_str(),
            world.account,
            "an installation sees only its own account's repositories"
        );
    }
}

#[then("authentication failure is reported without a completed listing")]
fn authentication_failure_is_reported_without_a_completed_listing(world: &ListingWorld) {
    assert!(
        world.reached_server,
        "the scenario must have reached the server to observe a rejection"
    );
    assert_eq!(
        world.outcome.as_ref().map(|outcome| outcome.clone().err()),
        Some(Some(RepositoryListingError::AuthenticationRejected)),
        "a rejected later page must fail the whole listing"
    );
}

#[then("the final repository is returned")]
fn the_final_repository_is_returned(world: &ListingWorld) {
    let Some(Ok(repositories)) = world.outcome.as_ref() else {
        panic!("the listing should have completed, got {:?}", world.outcome);
    };
    let observed = repositories
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>();
    assert_eq!(
        observed, world.expected,
        "the repository after an empty page must be returned"
    );
}

scenarios!("tests/features/repository_listing.feature", fixtures = [world: ListingWorld]);
