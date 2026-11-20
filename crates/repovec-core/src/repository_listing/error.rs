//! Semantic errors and resource budgets for repository listing.
//!
//! Every variant names a condition the caller can inspect and act on. Errors
//! never carry a credential, a raw response body, or a request URL, so they are
//! safe to print and to record in diagnostics.

use std::{fmt, time::Duration};

use thiserror::Error;

/// The moment, as a Unix timestamp in whole seconds, when a rate-limit window
/// resets.
///
/// A distinct type keeps a reset timestamp from being supplied where a retry
/// delay is expected, or the reverse. Both are whole seconds on the wire, so
/// exchanging them would otherwise pass unnoticed.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RateLimitReset(u64);

impl RateLimitReset {
    /// Creates a reset moment from the timestamp GitHub reported.
    ///
    /// # Parameters
    /// - `unix_seconds`: Seconds since the Unix epoch, as sent in `X-RateLimit-Reset`.
    ///
    /// # Returns
    /// The reset moment carrying that timestamp.
    ///
    /// # Examples
    ///
    /// ```
    /// use repovec_core::repository_listing::RateLimitReset;
    ///
    /// let reset = RateLimitReset::from_unix_seconds(1_700_000_000);
    ///
    /// assert_eq!(
    ///     reset.as_unix_seconds(),
    ///     1_700_000_000,
    ///     "the reset moment should keep its timestamp"
    /// );
    /// ```
    #[must_use]
    pub const fn from_unix_seconds(unix_seconds: u64) -> Self { Self(unix_seconds) }

    /// Returns the timestamp, in whole seconds, when the window resets.
    ///
    /// # Returns
    /// Seconds since the Unix epoch.
    ///
    /// # Examples
    ///
    /// ```
    /// use repovec_core::repository_listing::RateLimitReset;
    ///
    /// assert_eq!(
    ///     RateLimitReset::from_unix_seconds(7).as_unix_seconds(),
    ///     7,
    ///     "the accessor should return the timestamp"
    /// );
    /// ```
    #[must_use]
    pub const fn as_unix_seconds(self) -> u64 { self.0 }
}

/// Server-supplied rate-limit retry metadata.
///
/// Both values are optional because GitHub reports primary limits through
/// `X-RateLimit-Reset` and secondary limits through `Retry-After`, and either
/// may be absent. Only validated numeric values are retained; arbitrary
/// response text is discarded.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct RateLimitInfo {
    /// Delay requested by a `Retry-After` header.
    retry_after: Option<Duration>,
    /// Moment when the rate-limit window resets.
    reset: Option<RateLimitReset>,
}

impl RateLimitInfo {
    /// Creates rate-limit metadata from the values the server supplied.
    ///
    /// # Parameters
    /// - `retry_after`: Delay the server asked for, absent when it sent no usable `Retry-After`
    ///   header.
    /// - `reset`: Moment the rate-limit window resets, absent when the server sent no usable
    ///   `X-RateLimit-Reset` header.
    ///
    /// # Returns
    /// The metadata, carrying only the values that were supplied.
    ///
    /// # Examples
    ///
    /// ```
    /// use std::time::Duration;
    ///
    /// use repovec_core::repository_listing::{RateLimitInfo, RateLimitReset};
    ///
    /// let info = RateLimitInfo::new(
    ///     Some(Duration::from_secs(30)),
    ///     Some(RateLimitReset::from_unix_seconds(1_700_000_000)),
    /// );
    ///
    /// assert_eq!(
    ///     info.retry_after(),
    ///     Some(Duration::from_secs(30)),
    ///     "the delay should be kept"
    /// );
    /// ```
    #[must_use]
    pub const fn new(retry_after: Option<Duration>, reset: Option<RateLimitReset>) -> Self {
        Self { retry_after, reset }
    }

    /// Returns the delay the server asked for.
    ///
    /// # Returns
    /// The requested wait, or `None` when no usable `Retry-After` header was
    /// supplied.
    ///
    /// # Examples
    ///
    /// ```
    /// use std::time::Duration;
    ///
    /// use repovec_core::repository_listing::RateLimitInfo;
    ///
    /// let info = RateLimitInfo::new(Some(Duration::from_secs(5)), None);
    ///
    /// assert_eq!(
    ///     info.retry_after(),
    ///     Some(Duration::from_secs(5)),
    ///     "the accessor should return the delay"
    /// );
    /// ```
    #[must_use]
    pub const fn retry_after(self) -> Option<Duration> { self.retry_after }

    /// Returns the moment when the rate-limit window resets.
    ///
    /// # Returns
    /// The reset moment, or `None` when no usable `X-RateLimit-Reset` header
    /// was supplied.
    ///
    /// # Examples
    ///
    /// ```
    /// use repovec_core::repository_listing::{RateLimitInfo, RateLimitReset};
    ///
    /// let info = RateLimitInfo::new(None, Some(RateLimitReset::from_unix_seconds(9)));
    ///
    /// assert_eq!(
    ///     info.reset(),
    ///     Some(RateLimitReset::from_unix_seconds(9)),
    ///     "the accessor should return the reset moment"
    /// );
    /// ```
    #[must_use]
    pub const fn reset(self) -> Option<RateLimitReset> { self.reset }

    /// Returns whether the server supplied any retry metadata.
    ///
    /// # Returns
    /// `true` when at least one retry value was supplied.
    ///
    /// # Examples
    ///
    /// ```
    /// use std::time::Duration;
    ///
    /// use repovec_core::repository_listing::RateLimitInfo;
    ///
    /// assert!(
    ///     !RateLimitInfo::default().has_retry_metadata(),
    ///     "metadata built from no value should report none"
    /// );
    /// assert!(
    ///     RateLimitInfo::new(Some(Duration::from_secs(1)), None).has_retry_metadata(),
    ///     "a supplied delay should count as retry metadata"
    /// );
    /// ```
    #[must_use]
    pub const fn has_retry_metadata(self) -> bool {
        self.retry_after.is_some() || self.reset.is_some()
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
    ///     "the category identifier should be stable"
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
