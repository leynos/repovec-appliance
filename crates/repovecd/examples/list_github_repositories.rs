//! One-shot example: list the GitHub repositories one principal can see.
//!
//! This is the operator-observable acceptance surface for roadmap item 2.2.1.
//! It acquires one credential through the mode named on the command line,
//! drives one complete listing traversal with fixed documented budgets, and
//! prints one `owner/name` line per observation only after the traversal
//! succeeded. It is a diagnostics example and library demonstration, not a new
//! `repovectl` command.
//!
//! # Examples
//!
//! ```text
//! cargo run -p repovecd --example list_github_repositories -- --help
//! cargo run -p repovecd --example list_github_repositories -- \
//!     --user-store /etc/repovec
//! ```

use std::{
    ffi::OsString,
    io::{self, Write},
    path::PathBuf,
    process::ExitCode,
};

use camino::{Utf8Path, Utf8PathBuf};
use repovec_core::github_oauth::AccessToken;
use repovecd::{
    github_repository_client::{GitHubApiEndpoint, ListingLimits},
    github_token_store::{EncryptedGitHubTokenStore, SystemCommandRunner, SystemdCredsEncryptor},
    repository_listing_command::{
        CredentialError,
        ListingMode,
        ListingRun,
        UserTokenLoader,
        run_listing,
    },
};

/// Exit code for a command line the example cannot act on.
const USAGE_EXIT_CODE: u8 = 2;

/// What the arguments asked the example to do.
enum Invocation {
    /// Print the usage text and exit successfully.
    Help,
    /// Run one listing with this credential mode.
    List(ListingMode),
}

/// A usage error, already phrased for the operator.
struct UsageError(&'static str);

/// Loads the user credential through the encrypted store and `systemd-creds`.
struct EncryptedUserTokens;

impl UserTokenLoader for EncryptedUserTokens {
    fn load_user_token(&self, root: &Utf8Path) -> Result<AccessToken, CredentialError> {
        let encryptor = SystemdCredsEncryptor::new(SystemCommandRunner);
        let store = EncryptedGitHubTokenStore::open(root, encryptor).map_err(|error| {
            CredentialError::UserStore {
                reason: error.to_string(),
            }
        })?;
        store
            .load_token()
            .map_err(|error| CredentialError::UserStore {
                reason: error.to_string(),
            })
    }
}

fn main() -> ExitCode {
    let arguments: Vec<OsString> = std::env::args_os().skip(1).collect();
    let process_stdout = io::stdout();
    let mut stdout = process_stdout.lock();
    let process_stderr = io::stderr();
    let mut stderr = process_stderr.lock();
    match parse_arguments(&arguments) {
        Ok(Invocation::Help) => write_help(&mut stdout),
        Ok(Invocation::List(mode)) => run(mode, &mut stdout, &mut stderr),
        Err(error) => write_usage_error(&mut stderr, &error),
    }
}

/// Parses the small command line this example accepts.
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

/// Runs one listing against production GitHub with the documented budgets.
fn run(mode: ListingMode, stdout: &mut dyn Write, stderr: &mut dyn Write) -> ExitCode {
    let process_stdin = io::stdin();
    let mut stdin = process_stdin.lock();
    let loader = EncryptedUserTokens;
    let mut run = ListingRun {
        mode,
        endpoint: GitHubApiEndpoint::github(),
        limits: ListingLimits::defaults(),
        load_user_token: &loader,
        stdin: &mut stdin,
        stdout,
        stderr,
    };
    ExitCode::from(run_listing(&mut run).code())
}

/// Prints the usage text, including the budgets a run may consume.
fn write_help(stdout: &mut dyn Write) -> ExitCode {
    let written =
        writeln!(stdout, "{}", help_text(ListingLimits::defaults())).and_then(|()| stdout.flush());
    match written {
        Ok(()) => ExitCode::SUCCESS,
        Err(_) => ExitCode::from(5),
    }
}

/// Reports a command line the example cannot act on.
fn write_usage_error(stderr: &mut dyn Write, error: &UsageError) -> ExitCode {
    let usage = concat!(
        "\n\n",
        "Usage: list_github_repositories --user-store <DIR>\n",
        "       list_github_repositories --installation-token-stdin\n",
        "\n",
        "Run with --help for the full description.",
    );
    let written = writeln!(stderr, "list_github_repositories: {}{usage}", error.0)
        .and_then(|()| stderr.flush());
    match written {
        Ok(()) => ExitCode::from(USAGE_EXIT_CODE),
        Err(_) => ExitCode::from(6),
    }
}

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
