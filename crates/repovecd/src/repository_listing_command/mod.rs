//! One-shot composition of a GitHub repository listing run.
//!
//! The listing port and its adapter know nothing about where a credential comes
//! from or where observations go. This module supplies that composition: it
//! acquires exactly one credential, drives one completed traversal, writes one
//! `owner/name` line per observation only after the traversal succeeded, and
//! turns a failure into a sanitized diagnostic and a non-zero exit code.
//!
//! Everything crossing the boundary is injected. Credential loading, the API
//! origin, standard input, standard output, and standard error arrive from the
//! caller, so the runnable example and the offline end-to-end tests execute the
//! same composition without either reaching GitHub or mutating the process
//! environment.
//!
//! The [`cli`] submodule owns the operator command line: it parses the small
//! argument vector, prints the usage text, and maps every outcome to a
//! documented exit status. The runnable example supplies only the process
//! boundary to it, so the command-line behaviour is tested in-process.
//!
//! # Examples
//!
//! ```
//! use std::io::Cursor;
//!
//! use repovecd::{
//!     github_repository_client::{GitHubApiEndpoint, ListingLimits},
//!     repository_listing_command::{
//!         CredentialError,
//!         ListingMode,
//!         ListingRun,
//!         UserTokenLoader,
//!         run_listing,
//!     },
//! };
//!
//! struct NoUserStore;
//!
//! impl UserTokenLoader for NoUserStore {
//!     fn load_user_token(
//!         &self,
//!         _root: &camino::Utf8Path,
//!     ) -> Result<repovec_core::github_oauth::AccessToken, CredentialError> {
//!         Err(CredentialError::UserStore {
//!             reason: "not configured in this example".to_owned(),
//!         })
//!     }
//! }
//!
//! let mut stdin = Cursor::new(Vec::new());
//! let mut stdout = Vec::new();
//! let mut stderr = Vec::new();
//! let mut run = ListingRun {
//!     mode: ListingMode::InstallationStdin,
//!     endpoint: GitHubApiEndpoint::github(),
//!     limits: ListingLimits::defaults(),
//!     load_user_token: &NoUserStore,
//!     stdin: &mut stdin,
//!     stdout: &mut stdout,
//!     stderr: &mut stderr,
//! };
//!
//! // An empty standard input is rejected before any request is attempted.
//! let status = run_listing(&mut run);
//!
//! assert_eq!(status.code(), 3);
//! assert!(stdout.is_empty());
//! ```

pub mod cli;
mod credential;
mod output;
#[cfg(test)]
mod tests;

use std::{
    io::{self, Read, Write},
    time::Instant,
};

use camino::Utf8PathBuf;
pub use credential::{CredentialError, MAX_INSTALLATION_TOKEN_BYTES, UserTokenLoader};
use repovec_core::repository_listing::{
    RepositoryListing,
    RepositoryListingError,
    RepositorySummary,
};
use tracing::{Span, error, info, info_span};

use crate::github_repository_client::{
    GitHubApiEndpoint,
    GitHubCredential,
    GitHubRepositoryClient,
    ListingLimits,
};
pub use crate::github_rest::credential::PrincipalKind;

/// Where one listing run acquires its single credential.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ListingMode {
    /// Load the encrypted user credential stored below this directory.
    UserStore {
        /// Directory the encrypted credential is read from.
        root: Utf8PathBuf,
    },
    /// Read an already-issued installation token from standard input.
    InstallationStdin,
}

impl ListingMode {
    /// Returns the principal kind this mode authenticates as.
    ///
    /// # Examples
    ///
    /// ```
    /// use repovecd::repository_listing_command::{ListingMode, PrincipalKind};
    ///
    /// assert_eq!(
    ///     ListingMode::InstallationStdin.principal(),
    ///     PrincipalKind::Installation,
    /// );
    /// ```
    #[must_use]
    pub const fn principal(&self) -> PrincipalKind {
        match self {
            Self::UserStore { .. } => PrincipalKind::User,
            Self::InstallationStdin => PrincipalKind::Installation,
        }
    }
}

/// Everything one listing run needs from its composition root.
///
/// The streams are trait objects so a caller can pass the process streams, an
/// in-memory buffer, or a writer that fails on demand.
pub struct ListingRun<'a> {
    /// Credential source the run acquires before any request.
    pub mode: ListingMode,
    /// API origin the listing requests.
    pub endpoint: GitHubApiEndpoint,
    /// Budgets bounding the traversal.
    pub limits: ListingLimits,
    /// Loads a provisioned user credential when the mode names one.
    pub load_user_token: &'a dyn UserTokenLoader,
    /// Standard input the installation mode reads its token from.
    pub stdin: &'a mut dyn Read,
    /// Stream that receives one `owner/name` line per observation.
    pub stdout: &'a mut dyn Write,
    /// Stream that receives a sanitized diagnostic on failure.
    pub stderr: &'a mut dyn Write,
}

/// The outcome of one listing run, carrying the process exit code.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RunStatus {
    /// The traversal completed and every observation was written.
    Success,
    /// No credential could be acquired, so no request was attempted.
    CredentialFailure,
    /// The traversal failed, or the adapter rejected its configuration.
    ListingFailure,
    /// Writing observations failed; stdout may hold a partial prefix.
    OutputFailure,
    /// Writing a diagnostic failed, so the failure was reported only by status.
    DiagnosticFailure,
}

impl RunStatus {
    /// Returns whether the run completed and wrote every observation.
    ///
    /// # Examples
    ///
    /// ```
    /// use repovecd::repository_listing_command::RunStatus;
    ///
    /// assert!(RunStatus::Success.is_success());
    /// assert!(!RunStatus::ListingFailure.is_success());
    /// ```
    #[must_use]
    pub const fn is_success(self) -> bool { matches!(self, Self::Success) }

    /// Returns the process exit code this status maps to.
    ///
    /// Exit code 2 is reserved for a usage error, which
    /// [`cli::run`] decides before it composes a run.
    ///
    /// # Examples
    ///
    /// ```
    /// use repovecd::repository_listing_command::RunStatus;
    ///
    /// assert_eq!(RunStatus::Success.code(), 0);
    /// assert_eq!(RunStatus::ListingFailure.code(), 4);
    /// ```
    #[must_use]
    pub const fn code(self) -> u8 {
        match self {
            Self::Success => 0,
            Self::CredentialFailure => 3,
            Self::ListingFailure => 4,
            Self::OutputFailure => 5,
            Self::DiagnosticFailure => 6,
        }
    }
}

/// Why a run did not produce observations.
enum RunFailure {
    /// No credential could be acquired.
    Credential(CredentialError),
    /// The adapter rejected its configuration or the traversal failed.
    Listing(RepositoryListingError),
}

impl RunFailure {
    /// Returns the run status this failure is reported as.
    const fn status(&self) -> RunStatus {
        match self {
            Self::Credential(_) => RunStatus::CredentialFailure,
            Self::Listing(_) => RunStatus::ListingFailure,
        }
    }

    /// Returns the stable category recorded for this failure.
    const fn category(&self) -> &'static str {
        match self {
            Self::Credential(_) => "credential_failure",
            Self::Listing(error) => error.category(),
        }
    }
}

/// Elapsed time and the span every event of one run is reported under.
struct RunContext<'a> {
    /// Span the run's events are parented to.
    span: &'a Span,
    /// Instant the run started.
    started: Instant,
}

impl RunContext<'_> {
    /// Returns elapsed whole milliseconds, saturating rather than wrapping.
    fn elapsed_ms(&self) -> u64 {
        u64::try_from(self.started.elapsed().as_millis()).unwrap_or(u64::MAX)
    }
}

/// Acquires one credential, runs one completed traversal, and writes it out.
///
/// A failed traversal writes nothing to stdout. An output failure can leave a
/// prefix of the observations on stdout, because a stream cannot be rolled
/// back once a line has been written.
#[must_use]
pub fn run_listing(run: &mut ListingRun<'_>) -> RunStatus {
    let principal = run.mode.principal();
    let context = RunContext {
        span: &info_span!("repository_listing.run", principal = principal.as_str()),
        started: Instant::now(),
    };
    record_start(&context);
    match attempt(run) {
        Ok(repositories) => {
            record_completion(&context, repositories.len());
            write_output(run, &repositories, &context)
        }
        Err(failure) => report_failure(run, principal, &failure, &context),
    }
}

/// Records that one run has started authenticating as its principal.
fn record_start(context: &RunContext<'_>) {
    info!(parent: context.span, "repository listing run started");
}

/// Records a completed run with its accepted record count.
fn record_completion(context: &RunContext<'_>, records: usize) {
    info!(
        parent: context.span,
        records,
        elapsed_ms = context.elapsed_ms(),
        "repository listing run completed"
    );
}

/// Acquires the credential and runs the adapter for its principal.
fn attempt(run: &mut ListingRun<'_>) -> Result<Vec<RepositorySummary>, RunFailure> {
    let credential = credential::acquire(&run.mode, run.load_user_token, run.stdin)
        .map_err(RunFailure::Credential)?;
    list(&run.endpoint, credential, run.limits).map_err(RunFailure::Listing)
}

/// Runs the adapter for the credential's principal and returns its observations.
fn list(
    endpoint: &GitHubApiEndpoint,
    credential: GitHubCredential,
    limits: ListingLimits,
) -> Result<Vec<RepositorySummary>, RepositoryListingError> {
    GitHubRepositoryClient::new(endpoint.clone(), credential, limits)?.list_repositories()
}

/// Writes the observations and maps the outcome to a status.
fn write_output(
    run: &mut ListingRun<'_>,
    repositories: &[RepositorySummary],
    context: &RunContext<'_>,
) -> RunStatus {
    match output::write_observations(run.stdout, repositories) {
        Ok(()) => RunStatus::Success,
        Err(failure) => {
            error!(
                parent: context.span,
                category = "output_failure",
                elapsed_ms = context.elapsed_ms(),
                "repository listing output failed"
            );
            diagnostic_status(
                output::write_output_failure(run.stderr, &failure),
                RunStatus::OutputFailure,
            )
        }
    }
}

/// Reports a failed run and returns the status it maps to.
fn report_failure(
    run: &mut ListingRun<'_>,
    principal: PrincipalKind,
    failure: &RunFailure,
    context: &RunContext<'_>,
) -> RunStatus {
    error!(
        parent: context.span,
        category = failure.category(),
        elapsed_ms = context.elapsed_ms(),
        "repository listing run failed"
    );
    diagnostic_status(write_diagnostic(run, principal, failure), failure.status())
}

/// Writes the diagnostic that matches the failure.
fn write_diagnostic(
    run: &mut ListingRun<'_>,
    principal: PrincipalKind,
    failure: &RunFailure,
) -> io::Result<()> {
    match failure {
        RunFailure::Credential(error) => output::write_credential_failure(run.stderr, error),
        RunFailure::Listing(error) => output::write_listing_failure(run.stderr, principal, error),
    }
}

/// Maps whether the diagnostic reached its stream to the run status.
///
/// A failed diagnostic write is reported on its own, because the operator no
/// longer has the message that explains the earlier failure.
fn diagnostic_status(written: io::Result<()>, reported: RunStatus) -> RunStatus {
    written.map_or(RunStatus::DiagnosticFailure, |()| reported)
}
