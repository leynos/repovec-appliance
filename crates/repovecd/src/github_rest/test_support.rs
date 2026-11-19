//! Test-only access to crate-private listing internals.
//!
//! Integration tests live outside the crate, but some contracts can only be
//! observed from inside it. The third endpoint profile is the clearest case:
//! proving the traversal carries no repository knowledge means running it
//! against a path, filter set, and envelope key that no repository endpoint
//! uses. Hard-coding those values in a test would prove nothing about the
//! generic code, so the seam is exposed here instead and re-exported from
//! `github_repository_client` behind the `test-support` feature.
//!
//! This module is compiled only for `cfg(test)` or with the `test-support`
//! feature. It adds no behaviour and no production API.

use repovec_core::repository_listing::RepositoryListingError;
use reqwest::{Url, blocking::Client};
use serde::de::DeserializeOwned;

use super::{
    credential::GitHubCredential,
    limits::ListingLimits,
    profile::{EndpointProfile, PageItems},
    traversal::TraversalRunner,
};

/// Where a test profile's page body keeps its items.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TestPageItems {
    /// The body is a JSON array of items.
    BareArray,
    /// The body is a JSON object holding the items under this key.
    Envelope(&'static str),
}

impl TestPageItems {
    /// Returns the crate-private equivalent of this description.
    const fn as_page_items(self) -> PageItems {
        match self {
            Self::BareArray => PageItems::BareArray,
            Self::Envelope(key) => PageItems::Envelope(key),
        }
    }
}

/// The records and accounting one completed generic traversal observed.
#[derive(Debug)]
pub struct TraversalEvidence<T> {
    /// Decoded records in request and page order.
    pub records: Vec<T>,
    /// Accepted page count.
    pub pages: u64,
    /// Accepted response-body bytes.
    pub bytes: u64,
}

/// Describes one test-only endpoint profile.
#[derive(Clone, Copy, Debug)]
pub struct TestProfile {
    /// Path below the API origin.
    pub path: &'static str,
    /// Query pairs fixed for the whole traversal.
    pub fixed_query: &'static [(&'static str, &'static str)],
    /// Where a page keeps its items.
    pub items: TestPageItems,
}

/// Everything one test traversal needs apart from its starting URL.
#[derive(Clone, Copy, Debug)]
pub struct TestTraversal<'a> {
    /// HTTP client reused for every page.
    pub http: &'a Client,
    /// Credential presented on every request.
    pub credential: &'a GitHubCredential,
    /// Endpoint description to page.
    pub profile: TestProfile,
    /// Budgets bounding the traversal.
    pub limits: ListingLimits,
}

/// Runs one traversal with a test-only profile against an arbitrary origin.
///
/// The caller supplies the already-built request URL, so a test can point a
/// profile at a loopback server without production code growing an origin
/// override.
///
/// # Errors
///
/// Returns the same typed failures a listing returns.
pub fn traverse<T: DeserializeOwned>(
    traversal: TestTraversal<'_>,
    initial: Url,
) -> Result<TraversalEvidence<T>, RepositoryListingError> {
    let profile = traversal.profile;
    let endpoint = EndpointProfile::<T>::new(
        profile.path,
        profile.fixed_query,
        profile.items.as_page_items(),
    );
    let runner = TraversalRunner {
        client: traversal.http,
        credential: traversal.credential,
        profile: &endpoint,
        initial,
        limits: traversal.limits,
    };
    let completed = runner.run()?;
    Ok(TraversalEvidence {
        records: completed.records,
        pages: completed.pages,
        bytes: completed.bytes,
    })
}

/// Builds a request URL from an origin, path, and query pairs.
///
/// # Errors
///
/// Returns [`RepositoryListingError::InvalidInput`] when the origin does not
/// parse as a URL.
pub fn request_url(
    origin: &str,
    path: &str,
    query: &[(&'static str, &'static str)],
) -> Result<Url, RepositoryListingError> {
    let mut url = Url::parse(origin).map_err(|_| RepositoryListingError::InvalidInput {
        field: "api endpoint",
        reason: "must be a URL",
    })?;
    url.set_path(path);
    url.query_pairs_mut().extend_pairs(query.iter().copied());
    Ok(url)
}
