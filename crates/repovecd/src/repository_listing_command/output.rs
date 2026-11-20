//! Writing a completed listing and the diagnostics for a failed run.
//!
//! Observations are written only after the traversal returned Ok, so a failure
//! never leaves a prefix of a listing on stdout. Diagnostics name a condition,
//! a stable category, and one actionable recovery step. They never contain the
//! credential, a repository name, a raw URL, or a response body.

use std::io::{self, Write};

use repovec_core::repository_listing::{
    RateLimitInfo,
    RepositoryListingError,
    RepositorySummary,
    ResourceBudget,
};

use super::CredentialError;
use crate::github_repository_client::PrincipalKind;

/// Writes one `owner/name` line per observation.
///
/// Every write is fallible and the stream is flushed, so a failure is reported
/// rather than silently truncating a listing. Lines already written stay
/// written: a process cannot roll back a stream.
pub(super) fn write_observations(
    stdout: &mut dyn Write,
    repositories: &[RepositorySummary],
) -> io::Result<()> {
    for repository in repositories {
        writeln!(stdout, "{repository}")?;
    }
    stdout.flush()
}

/// Writes a credential-acquisition failure and its recovery step.
pub(super) fn write_credential_failure(
    stderr: &mut dyn Write,
    failure: &CredentialError,
) -> io::Result<()> {
    writeln!(stderr, "repository listing failed: {failure}")?;
    writeln!(stderr, "{}", credential_guidance(failure))?;
    stderr.flush()
}

/// Writes a failed traversal, its stable category, and any recovery step.
pub(super) fn write_listing_failure(
    stderr: &mut dyn Write,
    principal: PrincipalKind,
    failure: &RepositoryListingError,
) -> io::Result<()> {
    writeln!(
        stderr,
        "repository listing failed ({}): {failure}",
        failure.category()
    )?;
    if let Some(guidance) = recovery_guidance(principal, failure) {
        writeln!(stderr, "{guidance}")?;
    }
    stderr.flush()
}

/// Writes an output failure, which can follow a partial listing.
pub(super) fn write_output_failure(stderr: &mut dyn Write, failure: &io::Error) -> io::Result<()> {
    writeln!(
        stderr,
        "repository listing completed but its output could not be written: {failure}"
    )?;
    writeln!(
        stderr,
        "standard output may hold only part of the listing; retry with a writable destination"
    )?;
    stderr.flush()
}

/// Returns the recovery step for a credential acquisition failure.
const fn credential_guidance(failure: &CredentialError) -> &'static str {
    match failure {
        CredentialError::UserStore { .. } => concat!(
            "complete the GitHub device flow to provision a user credential, ",
            "then retry",
        ),
        CredentialError::StandardInput { .. } => concat!(
            "supply exactly one already-issued installation access token, ",
            "then end the input",
        ),
    }
}

/// Returns an actionable line for a failed traversal, when one is available.
fn recovery_guidance(principal: PrincipalKind, failure: &RepositoryListingError) -> Option<String> {
    match failure {
        RepositoryListingError::AuthenticationRejected => {
            Some(reauthentication(principal).to_owned())
        }
        RepositoryListingError::Forbidden => Some(
            "check the principal's repository access and organization policy, then retry"
                .to_owned(),
        ),
        RepositoryListingError::RateLimited { retry } => Some(rate_limit_guidance(*retry)),
        RepositoryListingError::TransportFailure => {
            Some("check connectivity to GitHub, then retry".to_owned())
        }
        RepositoryListingError::ResourceLimitExceeded { budget, limit } => {
            Some(limit_guidance(*budget, *limit))
        }
        RepositoryListingError::InvalidInput { .. } => {
            Some("correct the listing configuration, then retry".to_owned())
        }
        _ => None,
    }
}

/// Returns the reauthentication step for the principal that was rejected.
const fn reauthentication(principal: PrincipalKind) -> &'static str {
    match principal {
        PrincipalKind::User => concat!(
            "GitHub rejected the stored user credential; complete the device flow ",
            "again to store a fresh one",
        ),
        PrincipalKind::Installation => concat!(
            "GitHub rejected the installation token; obtain a fresh installation ",
            "token and supply it on standard input",
        ),
    }
}

/// Returns the wait a rate-limited caller can observe from validated metadata.
fn rate_limit_guidance(retry: RateLimitInfo) -> String {
    match (retry.retry_after(), retry.reset()) {
        (Some(delay), _) => {
            format!("wait at least {} seconds before retrying", delay.as_secs())
        }
        (None, Some(reset)) => format!(
            "wait until the rate-limit window resets at Unix time {}",
            reset.as_unix_seconds()
        ),
        (None, None) => "wait before retrying; GitHub supplied no retry metadata".to_owned(),
    }
}

/// Returns the recovery step for an exhausted traversal budget.
fn limit_guidance(budget: ResourceBudget, limit: u64) -> String {
    format!(
        "the {budget} budget of {limit} was exhausted; a caller can retry with larger validated \
         limits"
    )
}
