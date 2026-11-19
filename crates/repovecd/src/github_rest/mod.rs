//! Paginated GET traversal for GitHub REST listings.
//!
//! This module is private to `repovecd` and owns everything a GitHub listing
//! has in common: the API origin, the credential presented, the resource
//! budgets, `Link` continuation selection and confinement, cycle and progress
//! accounting, status classification, and bounded body reading. A listing is
//! described by an [`EndpointProfile`](profile::EndpointProfile) and decoded
//! wire records; adding a listing adds a profile, not traversal code.
//!
//! It owns no domain types, performs no retries, and schedules nothing. A
//! caller that needs several traversals decides its own whole-scan budget.

mod body;

pub(crate) mod credential;
pub(crate) mod limits;
mod link;
pub(crate) mod profile;
mod status;
#[cfg(any(test, feature = "test-support"))]
pub(crate) mod test_support;
pub(crate) mod traversal;

use repovec_core::repository_listing::RepositoryListingError;
use reqwest::Url;

/// The GitHub API origin used in production.
const GITHUB_ORIGIN: &str = "https://api.github.com/";

/// An API origin a listing may run against.
///
/// Production construction is [`GitHubApiEndpoint::github`]. The explicit
/// constructor exists so tests can inject a loopback server, and it refuses
/// plain HTTP for anything that is not a loopback address.
///
/// # Examples
///
/// ```
/// use repovecd::github_repository_client::GitHubApiEndpoint;
///
/// assert_eq!(
///     GitHubApiEndpoint::github().as_str(),
///     "https://api.github.com/"
/// );
///
/// let loopback = GitHubApiEndpoint::new("http://127.0.0.1:8080")?;
/// assert_eq!(loopback.as_str(), "http://127.0.0.1:8080/");
///
/// assert!(GitHubApiEndpoint::new("http://api.github.com").is_err());
///
/// # Ok::<(), repovec_core::repository_listing::RepositoryListingError>(())
/// ```
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GitHubApiEndpoint {
    /// Origin text, normalized by the URL parser.
    origin: String,
}

impl GitHubApiEndpoint {
    /// Returns the production GitHub API origin.
    #[must_use]
    pub fn github() -> Self {
        Self {
            origin: GITHUB_ORIGIN.to_owned(),
        }
    }

    /// Creates an endpoint from an explicit origin.
    ///
    /// # Errors
    ///
    /// Returns [`RepositoryListingError::InvalidInput`] naming `api endpoint`
    /// when the origin does not parse, carries user information, a query, or a
    /// fragment, or is plain HTTP to an address that is not loopback.
    pub fn new(origin: impl Into<String>) -> Result<Self, RepositoryListingError> {
        let parsed = Url::parse(&origin.into()).map_err(|_| invalid_endpoint("must be a URL"))?;
        if !parsed.username().is_empty() || parsed.password().is_some() {
            return Err(invalid_endpoint("must not carry user information"));
        }
        if parsed.query().is_some() || parsed.fragment().is_some() {
            return Err(invalid_endpoint("must not carry a query or fragment"));
        }
        if !is_permitted_origin(&parsed) {
            return Err(invalid_endpoint(
                "must use HTTPS unless the host is a loopback address",
            ));
        }
        Ok(Self {
            origin: parsed.to_string(),
        })
    }

    /// Returns the origin text.
    #[must_use]
    pub fn as_str(&self) -> &str { &self.origin }

    /// Builds the first request for a profile path and its fixed query.
    pub(crate) fn request_url(
        &self,
        path: &str,
        fixed_query: &[(&'static str, &'static str)],
    ) -> Result<Url, RepositoryListingError> {
        let mut url = Url::parse(&self.origin).map_err(|_| invalid_endpoint("must be a URL"))?;
        url.set_path(path);
        url.query_pairs_mut()
            .extend_pairs(fixed_query.iter().copied());
        Ok(url)
    }
}

/// Reports whether the parsed origin may receive a credential.
fn is_permitted_origin(url: &Url) -> bool {
    match url.scheme() {
        "https" => true,
        "http" => url.host_str().is_some_and(is_loopback_host),
        _ => false,
    }
}

/// Reports whether a host names this machine.
fn is_loopback_host(host: &str) -> bool {
    host == "localhost"
        || host == "::1"
        || host
            .parse::<std::net::Ipv4Addr>()
            .is_ok_and(|address| address.is_loopback())
}

/// Builds the typed failure for an unusable API origin.
const fn invalid_endpoint(reason: &'static str) -> RepositoryListingError {
    RepositoryListingError::InvalidInput {
        field: "api endpoint",
        reason,
    }
}
