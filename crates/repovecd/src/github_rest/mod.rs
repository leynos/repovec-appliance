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
///     "https://api.github.com/",
///     "the production endpoint should keep its documented origin"
/// );
///
/// let loopback = GitHubApiEndpoint::new("http://127.0.0.1:8080")?;
/// assert_eq!(
///     loopback.as_str(),
///     "http://127.0.0.1:8080/",
///     "a loopback endpoint should keep its normalized origin"
/// );
///
/// assert!(
///     GitHubApiEndpoint::new("http://api.github.com").is_err(),
///     "plain HTTP to a public host must be refused"
/// );
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
///
/// The URL parser serializes an IPv6 literal in brackets (`[::1]`), so the
/// brackets are removed before the address is parsed; comparing the raw host
/// text would reject a loopback endpoint the contract permits.
fn is_loopback_host(host: &str) -> bool {
    let address = host
        .strip_prefix('[')
        .and_then(|text| text.strip_suffix(']'))
        .unwrap_or(host);
    address == "localhost"
        || address
            .parse::<std::net::Ipv4Addr>()
            .is_ok_and(|parsed| parsed.is_loopback())
        || address
            .parse::<std::net::Ipv6Addr>()
            .is_ok_and(|parsed| parsed.is_loopback())
}

/// Builds the typed failure for an unusable API origin.
const fn invalid_endpoint(reason: &'static str) -> RepositoryListingError {
    RepositoryListingError::InvalidInput {
        field: "api endpoint",
        reason,
    }
}

#[cfg(test)]
mod tests {
    //! Origin-permission cases for [`GitHubApiEndpoint::new`].
    //!
    //! Plain HTTP is admitted only for a loopback host, whichever address
    //! family expresses it; HTTPS is admitted for any host. Each case names
    //! the host form and the outcome its contract requires.

    use rstest::rstest;

    use super::{GitHubApiEndpoint, is_loopback_host};

    #[rstest]
    #[case::ipv4_loopback("http://127.0.0.1:8080/")]
    #[case::ipv4_loopback_range("http://127.0.0.2:8080/")]
    #[case::ipv6_loopback("http://[::1]:8080/")]
    #[case::ipv6_loopback_without_port("http://[::1]/")]
    #[case::named_loopback("http://localhost:8080/")]
    fn plain_http_to_a_loopback_host_is_permitted(#[case] origin: &str) {
        let endpoint =
            GitHubApiEndpoint::new(origin).expect("a loopback origin over plain HTTP is permitted");

        assert_eq!(
            endpoint.as_str(),
            origin,
            "the endpoint must keep the origin text it accepted"
        );
    }

    #[rstest]
    #[case::public_ipv4("http://api.github.com/")]
    #[case::public_ipv6("http://[::2]/")]
    fn plain_http_to_a_non_loopback_host_is_refused(#[case] origin: &str) {
        let error = GitHubApiEndpoint::new(origin)
            .expect_err("plain HTTP to a non-loopback host must be refused");

        assert!(
            matches!(
                error,
                repovec_core::repository_listing::RepositoryListingError::InvalidInput {
                    field: "api endpoint",
                    ..
                }
            ),
            "{origin} must be refused as an invalid endpoint, got {error:?}"
        );
    }

    #[rstest]
    #[case::https_public_host("https://api.github.com/")]
    #[case::https_ipv6("https://[::2]/")]
    fn https_is_permitted_for_any_host(#[case] origin: &str) {
        GitHubApiEndpoint::new(origin)
            .expect("HTTPS is permitted for a host outside the loopback range");
    }

    #[test]
    fn every_loopback_address_form_is_recognized() {
        for host in ["localhost", "127.0.0.1", "127.1.2.3", "::1", "[::1]"] {
            assert!(
                is_loopback_host(host),
                "{host} names this machine and must be recognized as loopback"
            );
        }
        for host in ["api.github.com", "10.0.0.1", "::2", "[::2]"] {
            assert!(
                !is_loopback_host(host),
                "{host} is not loopback and must not be recognized as such"
            );
        }
    }
}
