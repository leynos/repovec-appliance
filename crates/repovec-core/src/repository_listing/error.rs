//! Semantic errors and resource budgets for repository listing.
//!
//! Every variant names a condition the caller can inspect and act on. Errors
//! never carry a credential, a raw response body, or a request URL, so they are
//! safe to print and to record in diagnostics.

use std::fmt;

use thiserror::Error;

/// Server-supplied rate-limit retry metadata.
///
/// Both fields are optional because GitHub reports primary limits through
/// `X-RateLimit-Reset` and secondary limits through `Retry-After`, and either
/// may be absent. Only validated numeric values are retained; arbitrary
/// response text is discarded.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct RateLimitInfo {
    /// Delay requested by a `Retry-After` header, in whole seconds.
    pub retry_after_seconds: Option<u64>,
    /// Unix timestamp, in seconds, when the rate-limit window resets.
    pub reset_unix_seconds: Option<u64>,
}

impl RateLimitInfo {
    /// Creates rate-limit metadata from optional validated values.
    ///
    /// # Examples
    ///
    /// ```
    /// use repovec_core::repository_listing::RateLimitInfo;
    ///
    /// let info = RateLimitInfo::new(Some(30), Some(1_700_000_000));
    ///
    /// assert_eq!(info.retry_after_seconds, Some(30));
    /// ```
    #[must_use]
    pub const fn new(retry_after_seconds: Option<u64>, reset_unix_seconds: Option<u64>) -> Self {
        Self {
            retry_after_seconds,
            reset_unix_seconds,
        }
    }

    /// Returns whether the server supplied any retry metadata.
    ///
    /// # Examples
    ///
    /// ```
    /// use repovec_core::repository_listing::RateLimitInfo;
    ///
    /// assert!(!RateLimitInfo::default().has_retry_metadata());
    /// assert!(RateLimitInfo::new(Some(1), None).has_retry_metadata());
    /// ```
    #[must_use]
    pub const fn has_retry_metadata(self) -> bool {
        self.retry_after_seconds.is_some() || self.reset_unix_seconds.is_some()
    }
}

/// A traversal budget that a successful listing must not cross.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ResourceBudget {
    /// Accepted pages in one traversal.
    Pages,
    /// Observed repository records in one traversal.
    Records,
    /// Decoded bytes of one response body.
    PageBytes,
    /// Cumulative accepted response-body bytes in one traversal.
    TotalBytes,
    /// Length of one `Link` header value.
    LinkHeaderBytes,
    /// Length of one canonical continuation identity.
    ContinuationIdentityBytes,
}

impl fmt::Display for ResourceBudget {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Pages => "page",
            Self::Records => "record",
            Self::PageBytes => "response body byte",
            Self::TotalBytes => "cumulative response body byte",
            Self::LinkHeaderBytes => "Link header byte",
            Self::ContinuationIdentityBytes => "continuation identity byte",
        })
    }
}

/// Errors returned while listing repositories.
///
/// The variants deliberately mirror the conditions a caller can distinguish:
/// bad input, rejected authority, throttling, transport failure, an unexpected
/// status, a malformed body or continuation, a continuation cycle, and an
/// exhausted traversal budget.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum RepositoryListingError {
    /// A supplied or configured input was invalid.
    #[error("invalid {field}: {reason}")]
    InvalidInput {
        /// Name of the offending input.
        field: &'static str,
        /// Stable description of why the input is invalid.
        reason: &'static str,
    },
    /// GitHub rejected the supplied credential.
    #[error("GitHub rejected the supplied credential; the principal must authenticate again")]
    AuthenticationRejected,
    /// The credential is valid but is not permitted to perform the request.
    #[error("GitHub forbids this request for the supplied principal")]
    Forbidden,
    /// GitHub refused the request because a rate limit was reached.
    #[error("GitHub rate limit reached; retry metadata: {retry:?}")]
    RateLimited {
        /// Validated retry metadata supplied by the server.
        retry: RateLimitInfo,
    },
    /// The request failed before a response was received.
    #[error("the repository listing request could not reach GitHub")]
    TransportFailure,
    /// GitHub returned a status the listing contract does not accept.
    #[error("GitHub returned unexpected status {status}")]
    UnexpectedStatus {
        /// HTTP status code returned by the server.
        status: u16,
    },
    /// A response body was not the expected JSON shape.
    #[error("a GitHub response body was invalid: {reason}")]
    InvalidResponse {
        /// Stable description of why the body is invalid.
        reason: &'static str,
    },
    /// A `Link` continuation was malformed or left the listing endpoint.
    #[error("a GitHub continuation link was invalid: {reason}")]
    InvalidContinuation {
        /// Stable description of why the continuation is invalid.
        reason: &'static str,
    },
    /// A continuation repeated a request already made in this traversal.
    #[error("the repository listing traversal revisited an earlier request")]
    CycleDetected,
    /// A traversal budget was exhausted.
    #[error("{budget} budget of {limit} exceeded")]
    ResourceLimitExceeded {
        /// Budget that would have been crossed.
        budget: ResourceBudget,
        /// Configured maximum for that budget.
        limit: u64,
    },
}

impl RepositoryListingError {
    /// Returns a short stable category for sanitized diagnostics and metrics.
    ///
    /// # Examples
    ///
    /// ```
    /// use repovec_core::repository_listing::RepositoryListingError;
    ///
    /// assert_eq!(
    ///     RepositoryListingError::AuthenticationRejected.category(),
    ///     "authentication_rejected",
    /// );
    /// ```
    #[must_use]
    pub const fn category(&self) -> &'static str {
        match self {
            Self::InvalidInput { .. } => "invalid_input",
            Self::AuthenticationRejected => "authentication_rejected",
            Self::Forbidden => "forbidden",
            Self::RateLimited { .. } => "rate_limited",
            Self::TransportFailure => "transport_failure",
            Self::UnexpectedStatus { .. } => "unexpected_status",
            Self::InvalidResponse { .. } => "invalid_response",
            Self::InvalidContinuation { .. } => "invalid_continuation",
            Self::CycleDetected => "cycle_detected",
            Self::ResourceLimitExceeded { .. } => "resource_limit_exceeded",
        }
    }
}
