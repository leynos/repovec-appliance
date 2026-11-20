//! Page-atomicity tests for the GitHub repository listing adapter.
//!
//! A listing either completes or fails: a caller may never receive the pages
//! that were accepted before a later failure. Each case scripts one accepted
//! page followed by a failing one and asserts the typed error, the exact
//! request count, and the absence of any listing value.

mod common;

use std::time::Duration;

use common::{installation_client::installation_client, user_client::user_client};
use pretty_assertions::assert_eq;
use repovec_core::repository_listing::{RateLimitInfo, RepositoryListing, RepositoryListingError};
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
use rstest::rstest;

/// The request deadline the atomicity tests allow themselves.
const TEST_TIMEOUT: Duration = Duration::from_secs(5);

/// The deadline used by the timeout case, well below the scripted delay.
const EXPIRING_TIMEOUT: Duration = Duration::from_millis(500);

/// The delay the timeout case scripts, well above its request deadline.
const SLOW_RESPONSE: Duration = Duration::from_secs(5);

/// Which credential kind an adapter under test presents.
///
/// The two profiles differ in both the page representation they accept and the
/// endpoint path they follow, so each case names one rather than repeating the
/// pairing at every call site.
#[derive(Clone, Copy, Debug)]
enum Profile {
    /// A user credential, whose pages are bare arrays.
    User,
    /// An installation credential, whose pages are enveloped.
    Installation,
}

impl Profile {
    /// Wraps one page's records in the representation this profile expects.
    fn page(self, records: &[String]) -> String {
        match self {
            Self::User => bare_page(records),
            Self::Installation => envelope_page(records),
        }
    }

    /// Returns the second-page URL the first page advertises as `next`.
    fn next_page(self) -> String {
        match self {
            Self::User => user_page(2),
            Self::Installation => installation_page(2),
        }
    }

    /// Builds an adapter for this profile at a loopback origin.
    fn client(
        self,
        origin: &str,
        deadline: Duration,
    ) -> Result<GitHubRepositoryClient, RepositoryListingError> {
        match self {
            Self::User => user_client(origin, ListingLimits::defaults(), deadline),
            Self::Installation => installation_client(origin, ListingLimits::defaults(), deadline),
        }
    }
}

/// One scripted later-page failure and the outcome it must produce.
struct LaterPageFailure {
    /// The credential profile the traversal runs under.
    profile: Profile,
    /// The request deadline the adapter is given.
    deadline: Duration,
    /// The response the second page returns.
    second: ScriptedResponse,
    /// The typed error the whole listing must report.
    expected: RepositoryListingError,
    /// Why this failure must not leak the accepted page, stated in the assertion.
    why: &'static str,
}

#[rstest]
#[case::server_error(LaterPageFailure {
    profile: Profile::User,
    deadline: TEST_TIMEOUT,
    second: ScriptedResponse::new(500),
    expected: RepositoryListingError::UnexpectedStatus { status: 500 },
    why: "two accepted records must not survive a failed third page as success",
})]
#[case::malformed_record(LaterPageFailure {
    profile: Profile::User,
    deadline: TEST_TIMEOUT,
    second: ScriptedResponse::new(200).with_body(bare_page(&[repository_json(Repository {
        id: 2,
        owner: "acme",
        name: "",
    })])),
    expected: RepositoryListingError::InvalidResponse {
        reason: "repository name is invalid",
    },
    why: "validation must cover every page, not only the first",
})]
#[case::rate_limited(LaterPageFailure {
    profile: Profile::User,
    deadline: TEST_TIMEOUT,
    second: ScriptedResponse::new(403)
        .with_header("X-RateLimit-Remaining", "0")
        .with_header("X-RateLimit-Reset", "1700000000"),
    expected: RepositoryListingError::RateLimited {
        retry: RateLimitInfo::new(None, Some(1_700_000_000)),
    },
    why: "a throttled later page must fail the listing with its retry metadata",
})]
#[case::timed_out(LaterPageFailure {
    profile: Profile::User,
    deadline: EXPIRING_TIMEOUT,
    second: ScriptedResponse::new(200).with_delay(SLOW_RESPONSE),
    expected: RepositoryListingError::TransportFailure,
    why: "a later page that outlives its deadline must fail the whole listing",
})]
#[case::installation_rejection(LaterPageFailure {
    profile: Profile::Installation,
    deadline: TEST_TIMEOUT,
    second: ScriptedResponse::new(401),
    expected: RepositoryListingError::AuthenticationRejected,
    why: "the enveloped profile must not return its accepted page either",
})]
#[case::accepted(LaterPageFailure {
    profile: Profile::User,
    deadline: TEST_TIMEOUT,
    second: ScriptedResponse::new(202).with_body(bare_page(&[repository_json(Repository {
        id: 2,
        owner: "acme",
        name: "two",
    })])),
    expected: RepositoryListingError::UnexpectedStatus { status: 202 },
    why: "a valid-looking 202 body is not the page the endpoint documents",
})]
#[case::partial_content(LaterPageFailure {
    profile: Profile::User,
    deadline: TEST_TIMEOUT,
    second: ScriptedResponse::new(206).with_body(bare_page(&[repository_json(Repository {
        id: 2,
        owner: "acme",
        name: "two",
    })])),
    expected: RepositoryListingError::UnexpectedStatus { status: 206 },
    why: "a valid-looking 206 body is not the page the endpoint documents",
})]
fn a_failure_on_a_later_page_returns_no_prefix(#[case] failure: LaterPageFailure) {
    let LaterPageFailure {
        profile,
        deadline,
        second,
        expected,
        why,
    } = failure;
    let first = profile.page(&[repository_json(Repository {
        id: 1,
        owner: "acme",
        name: "one",
    })]);
    let server = RecordingServer::start(vec![
        ScriptedResponse::new(200)
            .with_header(
                "Link",
                &link_header(&profile.next_page(), LinkRelation::Next),
            )
            .with_body(first),
        second,
    ])
    .expect("the scripted server should start");
    let client = profile
        .client(&server.origin(), deadline)
        .expect("an adapter for the named profile should construct");

    let result = client.list_repositories();

    assert_eq!(result.err(), Some(expected), "{why} (profile {profile:?})");
    assert_eq!(
        server.request_count(),
        2,
        "a failure on a later page must end the traversal without a retry"
    );
}
