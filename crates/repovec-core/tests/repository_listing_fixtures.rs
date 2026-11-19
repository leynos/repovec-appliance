//! Contract tests for the shared repository listing fixture builders.

use repovec_core::repository_listing::{
    RepositoryListing,
    RepositoryListingError,
    RepositorySummary,
};
use repovec_test_helpers::repository_listing::{
    RepositoryFixture,
    numbered_repositories,
    repository_summary,
};

/// A listing stub serving a fixed result through the public port.
struct StubListing(Result<Vec<RepositorySummary>, RepositoryListingError>);

impl RepositoryListing for StubListing {
    fn list_repositories(&self) -> Result<Vec<RepositorySummary>, RepositoryListingError> {
        self.0.clone()
    }
}

#[test]
fn default_builder_reports_a_public_repository_on_main() {
    let repository =
        repository_summary(1, "octocat", "hello-world").expect("the fixture should be valid");

    assert_eq!(repository.to_string(), "octocat/hello-world");
    assert!(!repository.is_private(), "the default should be public");
    assert_eq!(repository.default_branch(), Some("main"));
}

#[test]
fn explicit_builder_keeps_privacy_and_missing_default_branch() {
    let repository = RepositoryFixture::private(2, "acme", "private-svc")
        .build()
        .expect("the fixture should be valid");

    assert!(repository.is_private());
    assert_eq!(repository.default_branch(), None);
}

#[test]
fn numbered_builders_are_distinct_and_ordered() {
    let repositories = numbered_repositories("octocat", 3).expect("the fixtures should be valid");

    assert_eq!(
        repositories
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<String>>(),
        vec![
            "octocat/repo-1".to_owned(),
            "octocat/repo-2".to_owned(),
            "octocat/repo-3".to_owned(),
        ],
    );
}

#[test]
fn fixture_builders_reject_invalid_identifiers() {
    assert!(repository_summary(0, "octocat", "hello-world").is_err());
    assert!(repository_summary(1, "", "hello-world").is_err());
    assert!(repository_summary(1, "octocat", "").is_err());
    assert!(numbered_repositories("", 1).is_err());
}

#[test]
fn consumer_receives_every_fixture_observation_in_order() {
    let expected = numbered_repositories("octocat", 2).expect("the fixtures should be valid");
    let listing = StubListing(Ok(expected.clone()));

    assert_eq!(
        listing
            .list_repositories()
            .expect("the stub should succeed"),
        expected,
        "the fixture observations should survive the port unchanged",
    );
}

#[test]
fn consumer_receives_no_observations_when_the_listing_fails() {
    let listing = StubListing(Err(RepositoryListingError::Forbidden));

    assert_eq!(
        listing.list_repositories(),
        Err(RepositoryListingError::Forbidden),
        "a failed listing must not return a partial prefix",
    );
}
