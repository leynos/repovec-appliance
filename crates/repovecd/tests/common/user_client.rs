//! Construction of a user-mode adapter for the listing tests.
//!
//! The user credential pages `GET /user/repos` with the owner, collaborator,
//! and organization-member affiliation. Tests bind it to a loopback origin and
//! a fake token, so the protocol checks observe a real request while no test
//! reaches GitHub.

use std::time::Duration;

use repovec_core::{github_oauth::AccessToken, repository_listing::RepositoryListingError};
use repovec_test_helpers::repository_listing_wire::USER_TOKEN;
use repovecd::github_repository_client::{
    GitHubApiEndpoint,
    GitHubCredential,
    GitHubRepositoryClient,
    ListingLimits,
};

use crate::common::blocking_client::blocking_client;

/// Builds a user-mode adapter pointed at a loopback origin.
///
/// # Errors
///
/// Returns the adapter's typed construction failure.
pub fn user_client(
    origin: &str,
    limits: ListingLimits,
    timeout: Duration,
) -> Result<GitHubRepositoryClient, RepositoryListingError> {
    let endpoint = GitHubApiEndpoint::new(origin)?;
    let credential = GitHubCredential::User(AccessToken::new(USER_TOKEN, ["repo"]));
    let http = blocking_client(timeout).map_err(|_| RepositoryListingError::InvalidInput {
        field: "http client",
        reason: "must be constructible",
    })?;
    GitHubRepositoryClient::with_http_client(endpoint, credential, limits, http)
}
