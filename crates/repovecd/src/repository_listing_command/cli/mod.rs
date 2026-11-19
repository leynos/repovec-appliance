//! Command-line surface for the one-shot repository listing.
//!
//! The runnable example is a thin adapter over this module: it supplies the
//! process streams, the argument vector, and the production credential loader,
//! and returns the status computed here. Keeping the parsing, the usage text,
//! and the exit-code mapping in the library lets them be tested in-process,
//! without building or starting a binary and without depending on Cargo's
//! artefact layout.
//!
//! # Examples
//!
//! ```
//! use std::{ffi::OsString, io::Cursor};
//!
//! use repovecd::repository_listing_command::{
//!     CredentialError,
//!     UserTokenLoader,
//!     cli::{CliRun, run},
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
//! let arguments = [OsString::from("--help")];
//! let mut stdin = Cursor::new(Vec::new());
//! let mut stdout = Vec::new();
//! let mut stderr = Vec::new();
//! let mut command_line = CliRun {
//!     arguments: &arguments,
//!     stdin: &mut stdin,
//!     stdout: &mut stdout,
//!     stderr: &mut stderr,
//!     load_user_token: &NoUserStore,
//! };
//!
//! // The usage text is available without a credential or a request.
//! assert_eq!(run(&mut command_line), 0);
//! assert!(String::from_utf8_lossy(&stdout).contains("--installation-token-stdin"));
//! ```

use std::{
    ffi::OsString,
    io::{Read, Write},
    path::PathBuf,
};

use camino::Utf8PathBuf;

use super::{ListingMode, ListingRun, RunStatus, UserTokenLoader, run_listing};
use crate::github_repository_client::{GitHubApiEndpoint, ListingLimits};

/// Exit status for a command line the listing cannot act on.
const USAGE_EXIT_CODE: u8 = 2;

/// What the arguments asked the command line to do.
enum Invocation {
    /// Print the usage text and exit successfully.
    Help,
    /// Run one listing with this credential mode.
    List(ListingMode),
}

/// A usage error, already phrased for the operator.
struct UsageError(&'static str);

/// Everything one command line needs from its process boundary.
pub struct CliRun<'a> {
    /// The arguments after the program name.
    pub arguments: &'a [OsString],
    /// Standard input the installation mode reads its token from.
    pub stdin: &'a mut dyn Read,
    /// Stream that receives the usage text or one `owner/name` line each.
    pub stdout: &'a mut dyn Write,
    /// Stream that receives a usage or failure diagnostic.
    pub stderr: &'a mut dyn Write,
    /// Loads a provisioned user credential when the mode names one.
    pub load_user_token: &'a dyn UserTokenLoader,
}

/// Runs the command line against production GitHub and returns its status.
///
/// The status is the process exit code the caller should return: 0 for the
/// usage text or a completed listing, 2 for a usage error, and the
/// [`RunStatus`] codes for everything a run can fail with.
///
/// # Examples
///
/// ```
/// use std::{ffi::OsString, io::Cursor};
///
/// use repovecd::repository_listing_command::{
///     CredentialError,
///     UserTokenLoader,
///     cli::{CliRun, run},
/// };
///
/// struct NoUserStore;
///
/// impl UserTokenLoader for NoUserStore {
///     fn load_user_token(
///         &self,
///         _root: &camino::Utf8Path,
///     ) -> Result<repovec_core::github_oauth::AccessToken, CredentialError> {
///         Err(CredentialError::UserStore {
///             reason: "not configured in this example".to_owned(),
///         })
///     }
/// }
///
/// let arguments = [OsString::from("--unexpected")];
/// let mut stdin = Cursor::new(Vec::new());
/// let mut stdout = Vec::new();
/// let mut stderr = Vec::new();
/// let mut command_line = CliRun {
///     arguments: &arguments,
///     stdin: &mut stdin,
///     stdout: &mut stdout,
///     stderr: &mut stderr,
///     load_user_token: &NoUserStore,
/// };
///
/// assert_eq!(run(&mut command_line), 2);
/// assert!(stdout.is_empty());
/// ```
#[must_use]
pub fn run(command_line: &mut CliRun<'_>) -> u8 {
    match parse_arguments(command_line.arguments) {
        Ok(Invocation::Help) => write_help(command_line.stdout),
        Ok(Invocation::List(mode)) => run_listing_once(command_line, mode),
        Err(error) => write_usage_error(command_line.stderr, &error),
    }
}

/// Parses the small command line this surface accepts.
fn parse_arguments(arguments: &[OsString]) -> Result<Invocation, UsageError> {
    let mut mode = None;
    let mut remaining = arguments.iter();
    while let Some(argument) = remaining.next() {
        match argument.to_str() {
            Some("--help" | "-h") => return Ok(Invocation::Help),
            Some("--installation-token-stdin") => {
                set_mode(&mut mode, ListingMode::InstallationStdin)?;
            }
            Some("--user-store") => {
                let value = remaining
                    .next()
                    .ok_or(UsageError("--user-store requires a directory"))?;
                let root = Utf8PathBuf::from_path_buf(PathBuf::from(value))
                    .map_err(|_| UsageError("--user-store must be valid UTF-8"))?;
                set_mode(&mut mode, ListingMode::UserStore { root })?;
            }
            Some(_) => return Err(UsageError("unknown argument")),
            None => return Err(UsageError("arguments must be valid UTF-8")),
        }
    }
    mode.map(Invocation::List)
        .ok_or(UsageError("exactly one credential mode is required"))
}

/// Records the one credential mode the command line named.
fn set_mode(mode: &mut Option<ListingMode>, requested: ListingMode) -> Result<(), UsageError> {
    if mode.is_some() {
        return Err(UsageError("exactly one credential mode is required"));
    }
    *mode = Some(requested);
    Ok(())
}

/// Runs one listing with the documented budgets and maps its status.
fn run_listing_once(command_line: &mut CliRun<'_>, mode: ListingMode) -> u8 {
    let mut run = ListingRun {
        mode,
        endpoint: GitHubApiEndpoint::github(),
        limits: ListingLimits::defaults(),
        load_user_token: command_line.load_user_token,
        stdin: &mut *command_line.stdin,
        stdout: &mut *command_line.stdout,
        stderr: &mut *command_line.stderr,
    };
    run_listing(&mut run).code()
}

/// Prints the usage text, including the budgets a run may consume.
fn write_help(stdout: &mut dyn Write) -> u8 {
    let written =
        writeln!(stdout, "{}", help_text(ListingLimits::defaults())).and_then(|()| stdout.flush());
    written.map_or(RunStatus::OutputFailure.code(), |()| {
        RunStatus::Success.code()
    })
}

/// Reports a command line the listing cannot act on.
fn write_usage_error(stderr: &mut dyn Write, error: &UsageError) -> u8 {
    let written = writeln!(stderr, "list_github_repositories: {}{USAGE_HINT}", error.0)
        .and_then(|()| stderr.flush());
    written.map_or(RunStatus::DiagnosticFailure.code(), |()| USAGE_EXIT_CODE)
}

/// Where to look for the accepted arguments after a usage error.
const USAGE_HINT: &str = concat!(
    "\n\n",
    "Usage: list_github_repositories --user-store <DIR>\n",
    "       list_github_repositories --installation-token-stdin\n",
    "\n",
    "Run with --help for the full description.",
);

/// Builds the usage text for the documented fixed budgets.
fn help_text(limits: ListingLimits) -> String {
    format!(
        concat!(
            "Usage: list_github_repositories --user-store <DIR>\n",
            "       list_github_repositories --installation-token-stdin\n",
            "\n",
            "Print one owner/name line per repository visible to one authenticated\n",
            "GitHub principal, after the complete listing traversal succeeds.\n",
            "\n",
            "--user-store <DIR>          Load the encrypted user credential stored\n",
            "                            below DIR, for example /etc/repovec.\n",
            "--installation-token-stdin  Read an already-issued installation access\n",
            "                            token from standard input. At most 16 KiB is\n",
            "                            read, and no token is written to disk.\n",
            "-h, --help                  Print this help.\n",
            "\n",
            "Exactly one mode is required. The traversal is bounded by {} pages,\n",
            "{} records, {} bytes per response body, and {} cumulative response\n",
            "bytes. Pages are requested sequentially, with no retries.\n",
            "An empty listing is success. The listing is not a snapshot: if access\n",
            "changes during a traversal it may miss or repeat a repository.\n",
            "\n",
            "Exit status: 0 success, 2 usage error, 3 credential failure, 4 listing\n",
            "failure, 5 output failure, 6 diagnostic failure.\n",
        ),
        limits.max_pages(),
        limits.max_records(),
        limits.max_page_bytes(),
        limits.max_total_bytes(),
    )
}

#[cfg(test)]
mod tests;
