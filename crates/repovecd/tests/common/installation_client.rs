//! Construction of an installation-mode adapter for the listing tests.
//!
//! The installation credential pages `GET /installation/repositories` and
//! reads its envelope. Tests bind it to a loopback origin and a fake token, so
//! the protocol checks observe a real request while no test reaches GitHub.

use std::time::Duration;

use repovec_core::repository_listing::RepositoryListingError;
use repovec_test_helpers::repository_listing_wire::INSTALLATION_TOKEN;
use repovecd::github_repository_client::{
    GitHubApiEndpoint,
    GitHubCredential,
    GitHubRepositoryClient,
    InstallationAccessToken,
    ListingLimits,
};

use crate::common::blocking_client::blocking_client;

/// Builds an installation-mode adapter pointed at a loopback origin.
///
/// # Errors
///
/// Returns the adapter's typed construction failure.
pub fn installation_client(
    origin: &str,
    limits: ListingLimits,
    timeout: Duration,
) -> Result<GitHubRepositoryClient, RepositoryListingError> {
    let endpoint = GitHubApiEndpoint::new(origin)?;
    let credential =
        GitHubCredential::Installation(InstallationAccessToken::new(INSTALLATION_TOKEN)?);
    let http = blocking_client(timeout).map_err(|_| RepositoryListingError::InvalidInput {
        field: "http client",
        reason: "must be constructible",
    })?;
    GitHubRepositoryClient::with_http_client(endpoint, credential, limits, http)
}
