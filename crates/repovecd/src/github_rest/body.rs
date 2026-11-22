//! Bounded reading of a response body.
//!
//! A GitHub listing response is untrusted input. Whether the length is
//! declared or chunked, the body is read through a hard cap so a hostile or
//! malfunctioning endpoint cannot make the daemon allocate without bound.

use std::io::Read;

use repovec_core::repository_listing::{RepositoryListingError, ResourceBudget};
use reqwest::blocking::Response;

/// Reads a response body, failing before the body exceeds `limit` bytes.
///
/// # Errors
///
/// Returns [`RepositoryListingError::TransportFailure`] when the body cannot be
/// read, and [`RepositoryListingError::ResourceLimitExceeded`] with
/// [`ResourceBudget::PageBytes`] when it is longer than `limit`.
pub(crate) fn read_bounded(
    response: Response,
    limit: u64,
) -> Result<Vec<u8>, RepositoryListingError> {
    if response
        .content_length()
        .is_some_and(|declared| declared > limit)
    {
        return Err(RepositoryListingError::ResourceLimitExceeded {
            budget: ResourceBudget::PageBytes,
            limit,
        });
    }
    let mut body = Vec::new();
    response
        .take(limit.saturating_add(1))
        .read_to_end(&mut body)
        .map_err(|_| RepositoryListingError::TransportFailure)?;
    if body.len() as u64 > limit {
        return Err(RepositoryListingError::ResourceLimitExceeded {
            budget: ResourceBudget::PageBytes,
            limit,
        });
    }
    Ok(body)
}
