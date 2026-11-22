//! One-shot example: list the GitHub repositories one principal can see.
//!
//! This is the operator-observable acceptance surface for roadmap item 2.2.1.
//! It supplies only the process boundary — the argument vector, the process
//! streams, and the production credential loader — to
//! `repovecd::repository_listing_command::cli`, which owns the argument
//! parsing, the usage text, and the exit-code mapping. The run acquires one
//! credential through the mode named on the command line, drives one complete
//! listing traversal with fixed documented budgets, and prints one
//! `owner/name` line per observation only after the traversal succeeded. It is
//! a diagnostics example and library demonstration, not a new `repovectl`
//! command.
//!
//! # Examples
//!
//! ```text
//! cargo run -p repovecd --example list_github_repositories -- --help
//! cargo run -p repovecd --example list_github_repositories -- \
//!     --user-store /etc/repovec
//! ```

use std::{ffi::OsString, io, process::ExitCode};

use camino::Utf8Path;
use repovec_core::github_oauth::AccessToken;
use repovecd::{
    github_token_store::{EncryptedGitHubTokenStore, SystemCommandRunner, SystemdCredsEncryptor},
    repository_listing_command::{
        CredentialError,
        UserTokenLoader,
        cli::{CliRun, run},
    },
};

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

/// Installs the subscriber the run's structured events are reported through.
///
/// Events go to standard error, leaving standard output for `owner/name`
/// lines. The level is `INFO`, the level the composition's own events use; a
/// process that already configured a subscriber keeps it.
fn install_event_subscriber() {
    let _ignored = tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .with_target(false)
        .with_writer(io::stderr)
        .try_init();
}

fn main() -> ExitCode {
    install_event_subscriber();
    let arguments: Vec<OsString> = std::env::args_os().skip(1).collect();
    let process_stdin = io::stdin();
    let mut stdin = process_stdin.lock();
    let process_stdout = io::stdout();
    let mut stdout = process_stdout.lock();
    let process_stderr = io::stderr();
    let mut stderr = process_stderr.lock();
    let loader = EncryptedUserTokens;
    let mut command_line = CliRun {
        arguments: &arguments,
        stdin: &mut stdin,
        stdout: &mut stdout,
        stderr: &mut stderr,
        load_user_token: &loader,
    };

    ExitCode::from(run(&mut command_line))
}
