//! Fixtures for driving the one-shot listing composition offline.
//!
//! The composition is the same function the runnable example calls. These
//! helpers supply its injectable parts: a credential loader backed by the real
//! encrypted store with a reversible test encryptor, a loader that records an
//! installation-mode leak, an output writer that fails on demand, and a runner
//! that binds a loopback server and buffers both streams.

use std::{
    error::Error,
    io::{self, Read, Write},
};

use camino::Utf8Path;
use repovec_core::github_oauth::AccessToken;
use repovec_test_helpers::recording_http::RecordingServer;
use repovecd::{
    github_repository_client::{GitHubApiEndpoint, ListingLimits},
    github_token_store::{CredentialEncryptor, EncryptedGitHubTokenStore},
    repository_listing_command::{
        CredentialError,
        ListingMode,
        ListingRun,
        RunStatus,
        UserTokenLoader,
        run_listing,
    },
};

/// A reversible test encryptor that provides no cryptographic security.
#[derive(Clone, Copy, Debug)]
pub struct ReversibleEncryptor;

impl CredentialEncryptor for ReversibleEncryptor {
    type Error = io::Error;

    fn encrypt(&self, plaintext: &[u8]) -> Result<Vec<u8>, Self::Error> {
        let mut ciphertext = plaintext.to_vec();
        ciphertext.reverse();
        Ok(ciphertext)
    }

    fn decrypt(&self, ciphertext: &[u8]) -> Result<Vec<u8>, Self::Error> {
        let mut plaintext = ciphertext.to_vec();
        plaintext.reverse();
        Ok(plaintext)
    }
}

/// Loads the user credential through the real encrypted store.
#[derive(Clone, Copy, Debug)]
pub struct EncryptedUserTokens;

impl UserTokenLoader for EncryptedUserTokens {
    fn load_user_token(&self, root: &Utf8Path) -> Result<AccessToken, CredentialError> {
        let store =
            EncryptedGitHubTokenStore::open(root, ReversibleEncryptor).map_err(|error| {
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

/// A loader whose call would mean installation mode read a user credential.
#[derive(Clone, Copy, Debug)]
pub struct UnusedUserTokens;

impl UserTokenLoader for UnusedUserTokens {
    fn load_user_token(&self, _root: &Utf8Path) -> Result<AccessToken, CredentialError> {
        Err(CredentialError::UserStore {
            reason: "installation mode must not read the user store".to_owned(),
        })
    }
}

/// A writer that refuses every byte past a fixed budget.
#[derive(Debug)]
pub struct ByteBudgetWriter {
    /// Bytes accepted so far.
    pub written: Vec<u8>,
    /// Total bytes the writer will accept.
    budget: usize,
}

impl ByteBudgetWriter {
    /// Creates a writer that accepts exactly `budget` bytes.
    #[must_use]
    pub const fn new(budget: usize) -> Self {
        Self {
            written: Vec::new(),
            budget,
        }
    }
}

impl Write for ByteBudgetWriter {
    fn write(&mut self, buffer: &[u8]) -> io::Result<usize> {
        if self.written.len().saturating_add(buffer.len()) > self.budget {
            return Err(io::Error::other("the stream is closed"));
        }
        self.written.extend_from_slice(buffer);
        Ok(buffer.len())
    }

    fn flush(&mut self) -> io::Result<()> { Ok(()) }
}

/// Runs the composition against a loopback server with buffered streams.
///
/// Returns the status and both streams as text. The caller asserts on all
/// three, so a test observes exactly what an operator would.
///
/// # Errors
///
/// Returns the construction or decoding failure of the fixture rather than
/// panicking, so a broken fixture is reported as a failed test in this helper.
pub fn run_buffered(
    server: &RecordingServer,
    mode: ListingMode,
    stdin: &mut dyn Read,
    loader: &dyn UserTokenLoader,
) -> Result<(RunStatus, String, String), Box<dyn Error>> {
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();
    let mut run = ListingRun {
        mode,
        endpoint: GitHubApiEndpoint::new(server.origin())?,
        limits: ListingLimits::defaults(),
        load_user_token: loader,
        stdin,
        stdout: &mut stdout,
        stderr: &mut stderr,
    };
    let status = run_listing(&mut run);
    Ok((
        status,
        String::from_utf8(stdout)?,
        String::from_utf8(stderr)?,
    ))
}
