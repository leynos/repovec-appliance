//! Paginated GET traversal shared by every GitHub REST listing.
//!
//! One completed traversal returns every decoded record, in request order and
//! with multiplicity preserved. A failure returns an error and no prefix: the
//! accumulator exists only inside this module and is moved into the success
//! value at the end, so a caller can never mistake a partial scan for a
//! complete one.

use std::collections::BTreeSet;

use repovec_core::repository_listing::{RepositoryListingError, ResourceBudget};
use reqwest::{
    StatusCode,
    Url,
    blocking::{Client, Response},
    header::{ACCEPT, HeaderValue, LINK, USER_AGENT},
};
use serde::de::DeserializeOwned;

use super::{
    body::read_bounded,
    credential::GitHubCredential,
    limits::ListingLimits,
    link::{ContinuationContext, continuation_identity, next_continuation},
    profile::{EndpointProfile, PageItems},
    status::{MAX_ERROR_BODY_BYTES, classify_error},
};

/// The GitHub REST API version this client requests.
pub(crate) const GITHUB_API_VERSION: &str = "2026-03-10";

/// The media type selecting GitHub's stable JSON representation.
const GITHUB_MEDIA_TYPE: &str = "application/vnd.github+json";

/// The product token identifying this client to GitHub.
const USER_AGENT_VALUE: &str = "repovecd";

/// The header that selects the API version.
const API_VERSION_HEADER: &str = "X-GitHub-Api-Version";

/// Every record one completed traversal observed.
#[derive(Debug)]
pub(crate) struct Traversal<T> {
    /// Decoded records, in request and page order, keeping multiplicity.
    pub(crate) records: Vec<T>,
    /// Accepted pages.
    pub(crate) pages: u64,
    /// Accepted response-body bytes.
    pub(crate) bytes: u64,
}

/// Work a traversal accepted, whether or not it completed.
///
/// Counts are safe to report: they describe how far a run got without naming
/// or carrying any accepted value.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) struct AcceptedWork {
    /// Pages accepted.
    pub(crate) pages: u64,
    /// Records accepted.
    pub(crate) records: u64,
}

/// A failed traversal, with the work it accepted before failing.
///
/// The records themselves are dropped, because a failed scan must never look
/// like a complete one. The counts survive so diagnostics can report how far a
/// failed traversal got without exposing any accepted value.
#[derive(Debug)]
pub(crate) struct TraversalFailure {
    /// The typed error explaining the failure.
    pub(crate) error: RepositoryListingError,
    /// Work accepted before the failure.
    pub(crate) accepted: AcceptedWork,
}

impl TraversalFailure {
    /// Pairs an error with the accumulator's accepted work.
    fn new<T>(error: RepositoryListingError, state: &Accumulator<T>) -> Self {
        Self {
            error,
            accepted: AcceptedWork {
                pages: state.pages,
                records: u64::try_from(state.records.len()).unwrap_or(u64::MAX),
            },
        }
    }
}

/// One paginated listing walk, bound to one client, credential and profile.
pub(crate) struct TraversalRunner<'a, T> {
    /// HTTP client reused for every page.
    pub(crate) client: &'a Client,
    /// Credential presented on every request.
    pub(crate) credential: &'a GitHubCredential,
    /// Description of the listing being paged.
    pub(crate) profile: &'a EndpointProfile<T>,
    /// The validated first request.
    pub(crate) initial: Url,
    /// Budgets bounding the traversal.
    pub(crate) limits: ListingLimits,
}

impl<T: DeserializeOwned> TraversalRunner<'_, T> {
    /// Runs one traversal to its terminal page, or fails without a prefix.
    ///
    /// # Errors
    ///
    /// Returns [`TraversalFailure`] for a rejected status, an invalid
    /// continuation, a cycle, a transport failure, a malformed body, or an
    /// exhausted budget. The accepted counts travel with the error so the
    /// failure can be reported without exposing any accepted record.
    pub(crate) fn run(&self) -> Result<Traversal<T>, TraversalFailure> {
        let context = self.context();
        let mut state = Accumulator::new();
        let mut current = Some(self.initial.clone());
        while let Some(url) = current {
            current = self
                .fetch_page(&url, &context, &mut state)
                .map_err(|error| TraversalFailure::new(error, &state))?;
        }
        Ok(state.finish())
    }

    /// Builds the confinement rules every continuation is checked against.
    const fn context(&self) -> ContinuationContext<'_> {
        ContinuationContext {
            initial: &self.initial,
            fixed_query: self.profile.fixed_query,
            max_header_bytes: self.limits.max_link_header_bytes(),
            max_identity_bytes: self.limits.max_continuation_identity_bytes(),
        }
    }

    /// Requests one page, validates it, and folds it into the accumulator.
    fn fetch_page(
        &self,
        url: &Url,
        context: &ContinuationContext<'_>,
        state: &mut Accumulator<T>,
    ) -> Result<Option<Url>, RepositoryListingError> {
        let limits = self.limits;
        let pages = state
            .pages
            .checked_add(1)
            .ok_or_else(|| limit_reached(ResourceBudget::Pages, limits.max_pages()))?;
        if pages > limits.max_pages() {
            return Err(limit_reached(ResourceBudget::Pages, limits.max_pages()));
        }
        let identity = continuation_identity(url, context)?;
        if !state.visited.insert(identity) {
            return Err(RepositoryListingError::CycleDetected);
        }

        let response = send(self.client, url, self.credential)?;
        let status = response.status();
        if !status.is_success() {
            return Err(classify_failure(status, response));
        }
        let links = response
            .headers()
            .get_all(LINK)
            .iter()
            .cloned()
            .collect::<Vec<HeaderValue>>();
        let body = read_bounded(response, limits.max_page_bytes())?;

        let page_bytes = u64::try_from(body.len()).unwrap_or(u64::MAX);
        let bytes = state
            .bytes
            .checked_add(page_bytes)
            .ok_or_else(|| limit_reached(ResourceBudget::TotalBytes, limits.max_total_bytes()))?;
        if bytes > limits.max_total_bytes() {
            return Err(limit_reached(
                ResourceBudget::TotalBytes,
                limits.max_total_bytes(),
            ));
        }
        let records = decode::<T>(&body, self.profile.items)?;
        let counted = u64::try_from(records.len()).unwrap_or(u64::MAX);
        let total = u64::try_from(state.records.len())
            .unwrap_or(u64::MAX)
            .checked_add(counted)
            .ok_or_else(|| limit_reached(ResourceBudget::Records, limits.max_records()))?;
        if total > limits.max_records() {
            return Err(limit_reached(ResourceBudget::Records, limits.max_records()));
        }

        // A page is accepted only once its status, body, and continuation have
        // all validated, so a failure here must not leave the page counted.
        let next = next_continuation(&links, url, context)?;
        state.records.extend(records);
        state.pages = pages;
        state.bytes = bytes;
        Ok(next)
    }
}

/// Classifies a rejected response, reading a bounded part of its error body.
///
/// GitHub signals a secondary rate limit only in the error message, so the
/// body is needed to tell a limit from a refusal. A body that cannot be read
/// or is over the cap contributes no evidence; the status still classifies.
fn classify_failure(status: StatusCode, response: Response) -> RepositoryListingError {
    let headers = response.headers().clone();
    let body = read_bounded(response, MAX_ERROR_BODY_BYTES).unwrap_or_default();
    classify_error(status, &headers, &body)
}

/// Accumulated observations and accepted bytes.
#[derive(Debug)]
struct Accumulator<T> {
    /// Records accepted so far.
    records: Vec<T>,
    /// Pages accepted so far.
    pages: u64,
    /// Bytes accepted so far.
    bytes: u64,
    /// Canonical identities of requests already made.
    visited: BTreeSet<String>,
}

impl<T> Accumulator<T> {
    /// Creates an empty accumulator.
    const fn new() -> Self {
        Self {
            records: Vec::new(),
            pages: 0,
            bytes: 0,
            visited: BTreeSet::new(),
        }
    }

    /// Moves the accumulated records into the success value.
    fn finish(self) -> Traversal<T> {
        Traversal {
            records: self.records,
            pages: self.pages,
            bytes: self.bytes,
        }
    }
}

/// Sends one authenticated listing request.
fn send(
    client: &Client,
    url: &Url,
    credential: &GitHubCredential,
) -> Result<Response, RepositoryListingError> {
    client
        .get(url.clone())
        .header(ACCEPT, GITHUB_MEDIA_TYPE)
        .header(USER_AGENT, USER_AGENT_VALUE)
        .header(API_VERSION_HEADER, GITHUB_API_VERSION)
        .bearer_auth(credential.secret())
        .send()
        .map_err(|_| RepositoryListingError::TransportFailure)
}

/// Decodes the records a page body holds.
fn decode<T: DeserializeOwned>(
    body: &[u8],
    items: PageItems,
) -> Result<Vec<T>, RepositoryListingError> {
    match items {
        PageItems::BareArray => serde_json::from_slice(body)
            .map_err(|_| malformed("page body is not a JSON array of records")),
        PageItems::Envelope(key) => decode_envelope(body, key),
    }
}

/// Decodes records held under an envelope key.
fn decode_envelope<T: DeserializeOwned>(
    body: &[u8],
    key: &str,
) -> Result<Vec<T>, RepositoryListingError> {
    let envelope = serde_json::from_slice::<serde_json::Map<String, serde_json::Value>>(body)
        .map_err(|_| malformed("page body is not a JSON object"))?;
    let records = envelope
        .get(key)
        .ok_or_else(|| malformed("page body is missing its records"))?;
    serde_json::from_value(records.clone())
        .map_err(|_| malformed("page records do not match their schema"))
}

/// Builds the typed failure for an exhausted budget.
const fn limit_reached(budget: ResourceBudget, limit: u64) -> RepositoryListingError {
    RepositoryListingError::ResourceLimitExceeded { budget, limit }
}

/// Builds the typed failure for a body that does not match its schema.
const fn malformed(reason: &'static str) -> RepositoryListingError {
    RepositoryListingError::InvalidResponse { reason }
}
