//! Classification of a GitHub REST response status.
//!
//! Status classification is deliberately evidence-based. A `403` is only a
//! rate limit when GitHub says so, in a header or in the error message it
//! documents for secondary limits; otherwise it is a refusal of access.
//! Nothing here retries, prompts for a login, or falls back to public data.

use std::time::Duration;

use repovec_core::repository_listing::{RateLimitInfo, RateLimitReset, RepositoryListingError};
use reqwest::{
    StatusCode,
    header::{HeaderMap, RETRY_AFTER},
};

/// The most error-body bytes this module reads to classify a failure.
///
/// GitHub's documented secondary-limit signal is a short error message. The
/// cap keeps a hostile or malfunctioning endpoint from making failure
/// handling allocate without bound, and classification never depends on the
/// body being readable.
pub(crate) const MAX_ERROR_BODY_BYTES: u64 = 8 * 1024;

/// GitHub's primary rate-limit reset time, in Unix seconds.
const RATE_LIMIT_RESET_HEADER: &str = "x-ratelimit-reset";

/// GitHub's primary rate-limit remaining count.
const RATE_LIMIT_REMAINING_HEADER: &str = "x-ratelimit-remaining";

/// The wording GitHub's error message uses for every limit response.
const RATE_LIMIT_MESSAGE_MARKER: &str = "rate limit";

/// Classifies any status other than the documented page success, keeping only
/// bounded retry metadata.
///
/// A 2xx status other than `200` is not the page both listing endpoints
/// document, so it surfaces as [`RepositoryListingError::UnexpectedStatus`]
/// rather than as success.
///
/// # Errors
///
/// Always returns the error the status represents; the function exists so the
/// caller can write `return classify_error(status, headers, body)`.
pub(crate) fn classify_error(
    status: StatusCode,
    headers: &HeaderMap,
    body: &[u8],
) -> RepositoryListingError {
    match status.as_u16() {
        401 => RepositoryListingError::AuthenticationRejected,
        403 if has_rate_limit_evidence(headers, body) => rate_limited(headers),
        403 => RepositoryListingError::Forbidden,
        429 => rate_limited(headers),
        other => RepositoryListingError::UnexpectedStatus { status: other },
    }
}

/// Reports whether the response evidences a rate limit rather than a refusal.
///
/// GitHub includes the `x-ratelimit-*` headers on ordinary responses, so a
/// reset time alone says nothing about why a request was refused. A `403` is a
/// rate limit when the primary budget is exhausted (`remaining` is `0`), when
/// GitHub supplied a `Retry-After`, or when its error message reports a
/// secondary limit. GitHub documents the second signal only as "an error
/// message that indicates that you exceeded a secondary rate limit", and the
/// `Retry-After` header is optional for that case, so the message is the only
/// evidence available for some genuine limits. Any other `403` is an access
/// refusal, and telling the operator to wait would hide the real remedy.
fn has_rate_limit_evidence(headers: &HeaderMap, body: &[u8]) -> bool {
    headers.contains_key(RETRY_AFTER)
        || headers
            .get(RATE_LIMIT_REMAINING_HEADER)
            .is_some_and(|remaining| remaining.as_bytes() == b"0")
        || message_reports_rate_limit(body)
}

/// Reports whether a JSON error body's message names a rate limit.
///
/// Only the documented `message` field is read, and only for the fixed
/// wording GitHub uses; an unparsable body, or one without that field, is
/// not evidence and leaves the response a refusal.
fn message_reports_rate_limit(body: &[u8]) -> bool {
    let Ok(envelope) = serde_json::from_slice::<serde_json::Value>(body) else {
        return false;
    };
    envelope
        .get("message")
        .and_then(serde_json::Value::as_str)
        .is_some_and(|message| {
            message
                .to_ascii_lowercase()
                .contains(RATE_LIMIT_MESSAGE_MARKER)
        })
}

/// Extracts the retry metadata GitHub reported, if any.
fn rate_limited(headers: &HeaderMap) -> RepositoryListingError {
    RepositoryListingError::RateLimited {
        retry: RateLimitInfo::new(retry_after(headers), reset_unix_seconds(headers)),
    }
}

/// Returns the delay in `Retry-After`, ignoring a date form.
fn retry_after(headers: &HeaderMap) -> Option<Duration> {
    headers
        .get(RETRY_AFTER)?
        .to_str()
        .ok()?
        .trim()
        .parse()
        .ok()
        .map(Duration::from_secs)
}

/// Returns the reset moment reported by GitHub.
fn reset_unix_seconds(headers: &HeaderMap) -> Option<RateLimitReset> {
    headers
        .get(RATE_LIMIT_RESET_HEADER)?
        .to_str()
        .ok()?
        .trim()
        .parse()
        .ok()
        .map(RateLimitReset::from_unix_seconds)
}

#[cfg(test)]
mod tests;
