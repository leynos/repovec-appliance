//! Classification of a GitHub REST response status.
//!
//! Status classification is deliberately evidence-based. A `403` is only a
//! rate limit when GitHub says so in a header; otherwise it is a refusal of
//! access. Nothing here retries, prompts for a login, or falls back to public
//! data.

use repovec_core::repository_listing::{RateLimitInfo, RepositoryListingError};
use reqwest::{
    StatusCode,
    header::{HeaderMap, RETRY_AFTER},
};

/// GitHub's primary rate-limit reset time, in Unix seconds.
const RATE_LIMIT_RESET_HEADER: &str = "x-ratelimit-reset";

/// GitHub's primary rate-limit remaining count.
const RATE_LIMIT_REMAINING_HEADER: &str = "x-ratelimit-remaining";

/// Classifies a non-success status, keeping only bounded retry metadata.
///
/// # Errors
///
/// Always returns the error the status represents; the function exists so the
/// caller can write `return classify_error(status, headers)`.
pub(crate) fn classify_error(status: StatusCode, headers: &HeaderMap) -> RepositoryListingError {
    match status.as_u16() {
        401 => RepositoryListingError::AuthenticationRejected,
        403 if has_rate_limit_evidence(headers) => rate_limited(headers),
        403 => RepositoryListingError::Forbidden,
        429 => rate_limited(headers),
        other => RepositoryListingError::UnexpectedStatus { status: other },
    }
}

/// Reports whether the headers evidence a rate limit rather than a refusal.
fn has_rate_limit_evidence(headers: &HeaderMap) -> bool {
    headers.contains_key(RETRY_AFTER)
        || headers.contains_key(RATE_LIMIT_RESET_HEADER)
        || headers
            .get(RATE_LIMIT_REMAINING_HEADER)
            .is_some_and(|remaining| remaining.as_bytes() == b"0")
}

/// Extracts the retry metadata GitHub reported, if any.
fn rate_limited(headers: &HeaderMap) -> RepositoryListingError {
    RepositoryListingError::RateLimited {
        retry: RateLimitInfo::new(retry_after(headers), reset_unix_seconds(headers)),
    }
}

/// Returns the whole number of seconds in `Retry-After`, ignoring a date form.
fn retry_after(headers: &HeaderMap) -> Option<u64> {
    headers.get(RETRY_AFTER)?.to_str().ok()?.trim().parse().ok()
}

/// Returns the reset time in Unix seconds reported by GitHub.
fn reset_unix_seconds(headers: &HeaderMap) -> Option<u64> {
    headers
        .get(RATE_LIMIT_RESET_HEADER)?
        .to_str()
        .ok()?
        .trim()
        .parse()
        .ok()
}

#[cfg(test)]
mod tests;
