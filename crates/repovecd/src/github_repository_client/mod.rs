//! GitHub repository listing adapter.
//!
//! [`GitHubRepositoryClient`] binds one credential kind, one API origin, and
//! one set of resource budgets, and implements the domain
//! [`RepositoryListing`] port. A successful call returns every observation
//! from one completed traversal; a failure returns a typed error and no
//! listing at all.
//!
//! The adapter owns two endpoint profiles. A user credential pages
//! `GET /user/repos` with full visibility and an owner/collaborator/
//! organization affiliation, returning a bare JSON array. An installation
//! credential pages `GET /installation/repositories`, returning an envelope
//! whose `repositories` key holds the items. Page traversal, continuation
//! confinement, budgets, and status classification live in the crate-private
//! `github_rest` module.

mod wire;

use std::time::{Duration, Instant};

use repovec_core::repository_listing::{
    RepositoryListing,
    RepositoryListingError,
    RepositorySummary,
};
use reqwest::blocking::Client;
use tracing::{Span, info, info_span};
use wire::RepositoryWire;

#[cfg(any(test, feature = "test-support"))]
pub use crate::github_rest::test_support::{
    TestPageItems,
    TestProfile,
    TestTraversal,
    TraversalEvidence,
    request_url,
    traverse,
};
pub use crate::github_rest::{
    GitHubApiEndpoint,
    credential::{GitHubCredential, InstallationAccessToken, PrincipalKind},
    limits::ListingLimits,
};
use crate::github_rest::{
    profile::{EndpointProfile, PageItems},
    traversal::{AcceptedWork, Traversal, TraversalFailure, TraversalRunner},
};

/// Connection deadline applied by the default HTTP client.
pub const DEFAULT_CONNECT_TIMEOUT: Duration = Duration::from_secs(10);

/// Overall deadline applied to each request by the default HTTP client.
pub const DEFAULT_REQUEST_TIMEOUT: Duration = Duration::from_secs(30);

/// Repositories visible to the authenticated user.
const USER_REPOSITORIES: EndpointProfile<RepositoryWire> = EndpointProfile::new(
    "/user/repos",
    &[
        ("visibility", "all"),
        ("affiliation", "owner,collaborator,organization_member"),
        ("sort", "full_name"),
        ("direction", "asc"),
        ("per_page", "100"),
    ],
    PageItems::BareArray,
);

/// Repositories an installation was granted.
const INSTALLATION_REPOSITORIES: EndpointProfile<RepositoryWire> = EndpointProfile::new(
    "/installation/repositories",
    &[("per_page", "100")],
    PageItems::Envelope("repositories"),
);

/// Blocking GitHub repository listing adapter.
#[derive(Debug)]
pub struct GitHubRepositoryClient {
    /// HTTP client reused across every page of a traversal.
    http: Client,
    /// Validated API origin.
    endpoint: GitHubApiEndpoint,
    /// Credential presented on every request.
    credential: GitHubCredential,
    /// Budgets bounding each traversal.
    limits: ListingLimits,
}

impl GitHubRepositoryClient {
    /// Creates an adapter with the production HTTP timeouts.
    ///
    /// # Errors
    ///
    /// Returns [`RepositoryListingError::InvalidInput`] when the origin,
    /// credential, or budgets are unusable, or when the HTTP client cannot be
    /// built.
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// use repovec_core::github_oauth::AccessToken;
    /// use repovecd::github_repository_client::{
    ///     GitHubApiEndpoint,
    ///     GitHubCredential,
    ///     GitHubRepositoryClient,
    ///     ListingLimits,
    /// };
    ///
    /// let client = GitHubRepositoryClient::new(
    ///     GitHubApiEndpoint::github(),
    ///     GitHubCredential::User(AccessToken::new("gho_token", ["repo"])),
    ///     ListingLimits::defaults(),
    /// )?;
    ///
    /// # Ok::<(), repovec_core::repository_listing::RepositoryListingError>(())
    /// ```
    pub fn new(
        endpoint: GitHubApiEndpoint,
        credential: GitHubCredential,
        limits: ListingLimits,
    ) -> Result<Self, RepositoryListingError> {
        let http = build_client(DEFAULT_CONNECT_TIMEOUT, DEFAULT_REQUEST_TIMEOUT)?;
        Self::with_http_client(endpoint, credential, limits, http)
    }

    /// Creates an adapter around a caller-supplied HTTP client.
    ///
    /// Supply this when the default deadlines are unsuitable. The client must
    /// have redirects disabled: a redirect would move a request without the
    /// continuation checks ever seeing it. Tests use it to inject short
    /// deadlines instead of waiting out a production one.
    ///
    /// # Errors
    ///
    /// Returns [`RepositoryListingError::InvalidInput`] when the credential or
    /// budgets are unusable.
    pub fn with_http_client(
        endpoint: GitHubApiEndpoint,
        credential: GitHubCredential,
        limits: ListingLimits,
        http: Client,
    ) -> Result<Self, RepositoryListingError> {
        credential.validate()?;
        let validated = limits.validate()?;
        Ok(Self {
            http,
            endpoint,
            credential,
            limits: validated,
        })
    }

    /// Returns the API origin this adapter requests.
    #[must_use]
    pub const fn endpoint(&self) -> &GitHubApiEndpoint { &self.endpoint }

    /// Returns the principal kind this adapter authenticates as.
    #[must_use]
    pub const fn principal(&self) -> PrincipalKind { self.credential.principal() }

    /// Returns the budgets bounding each traversal.
    #[must_use]
    pub const fn limits(&self) -> ListingLimits { self.limits }
}

impl RepositoryListing for GitHubRepositoryClient {
    fn list_repositories(&self) -> Result<Vec<RepositorySummary>, RepositoryListingError> {
        let span = info_span!(
            "github_repository_listing.list",
            principal = self.credential.principal().as_str()
        );
        let started = Instant::now();
        let traversal = match self.traverse(&span) {
            Ok(traversal) => traversal,
            Err(failure) => return Err(report_traversal_failure(&span, started, failure)),
        };
        finish(&span, started, traversal)
    }
}

impl GitHubRepositoryClient {
    /// Runs the traversal for this credential's profile.
    fn traverse(&self, span: &Span) -> Result<Traversal<RepositoryWire>, TraversalFailure> {
        let profile = self.profile();
        let initial = span
            .in_scope(|| self.endpoint.request_url(profile.path, profile.fixed_query))
            .map_err(|error| TraversalFailure {
                error,
                accepted: AcceptedWork::default(),
            })?;
        let runner = TraversalRunner {
            client: &self.http,
            credential: &self.credential,
            profile,
            initial,
            limits: self.limits,
        };
        runner.run()
    }

    /// Returns the profile the bound credential kind pages.
    const fn profile(&self) -> &'static EndpointProfile<RepositoryWire> {
        match self.credential.principal() {
            PrincipalKind::User => &USER_REPOSITORIES,
            PrincipalKind::Installation => &INSTALLATION_REPOSITORIES,
        }
    }
}

/// Validates a completed traversal, then records it as complete.
///
/// Conversion runs first: a traversal that accepted pages but returned a
/// record the domain rejects has failed, and logging completion before the
/// validation would describe a failed run as a completed one.
fn finish(
    span: &Span,
    started: Instant,
    traversal: Traversal<RepositoryWire>,
) -> Result<Vec<RepositorySummary>, RepositoryListingError> {
    let accepted = AcceptedWork {
        pages: traversal.pages,
        records: u64::try_from(traversal.records.len()).unwrap_or(u64::MAX),
    };
    match into_domain(traversal.records) {
        Ok(repositories) => {
            record_completion(span, started, accepted, traversal.bytes);
            Ok(repositories)
        }
        Err(error) => {
            record_failure(span, started, &error, accepted);
            Err(error)
        }
    }
}

/// Records a failed traversal and returns the error it carries.
fn report_traversal_failure(
    span: &Span,
    started: Instant,
    failure: TraversalFailure,
) -> RepositoryListingError {
    record_failure(span, started, &failure.error, failure.accepted);
    failure.error
}

/// Converts the wire records of a completed traversal into domain summaries.
fn into_domain(
    records: Vec<RepositoryWire>,
) -> Result<Vec<RepositorySummary>, RepositoryListingError> {
    records
        .into_iter()
        .map(RepositoryWire::into_domain)
        .collect()
}

/// Records a completed traversal.
///
/// The event carries the same accepted page and record counts a failure event
/// reports, so an operator comparing a completed run with a failed one sees
/// the same vocabulary on both.
fn record_completion(span: &Span, started: Instant, accepted: AcceptedWork, bytes: u64) {
    info!(
        parent: span,
        pages = accepted.pages,
        records = accepted.records,
        bytes,
        elapsed_ms = elapsed_ms(started),
        "repository listing completed"
    );
}

/// Records a failed traversal under a stable category and its accepted work.
fn record_failure(
    span: &Span,
    started: Instant,
    error: &RepositoryListingError,
    accepted: AcceptedWork,
) {
    info!(
        parent: span,
        category = error.category(),
        pages = accepted.pages,
        records = accepted.records,
        elapsed_ms = elapsed_ms(started),
        "repository listing failed"
    );
}

/// Builds a client with redirects disabled and bounded deadlines.
fn build_client(connect: Duration, request: Duration) -> Result<Client, RepositoryListingError> {
    Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .connect_timeout(connect)
        .timeout(request)
        .build()
        .map_err(|_| RepositoryListingError::TransportFailure)
}

/// Returns elapsed whole milliseconds, saturating rather than wrapping.
fn elapsed_ms(started: Instant) -> u64 {
    u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX)
}
