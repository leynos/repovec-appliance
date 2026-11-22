//! Credential acquisition for one listing run.
//!
//! A run acquires exactly one credential before any request: either a
//! provisioned user credential loaded from its encrypted store, or an
//! already-issued installation token read from standard input. The kind is
//! chosen by the caller, never inferred from the token.
//!
//! Standard input is bounded. At most [`MAX_INSTALLATION_TOKEN_BYTES`] plus one
//! sentinel byte is read, one trailing line ending is removed, and the result
//! must be a header-safe bearer value. A literal token argument is not offered,
//! because a command line is visible to every process on the host.

use std::io::Read;

use camino::Utf8Path;
use repovec_core::{github_oauth::AccessToken, repository_listing::RepositoryListingError};
use thiserror::Error;

use super::ListingMode;
use crate::github_repository_client::{GitHubCredential, InstallationAccessToken};

/// Largest installation token accepted from standard input, in bytes.
///
/// The limit keeps a scripted or mistaken input from being buffered without
/// bound. A token is far shorter in practice.
pub const MAX_INSTALLATION_TOKEN_BYTES: usize = 16 * 1024;

/// A credential that could not be acquired before any request was attempted.
///
/// Both variants carry sanitized text only: no token, no store path, and no
/// raw adapter error chain reaches the diagnostic.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum CredentialError {
    /// The provisioned user credential could not be loaded or decrypted.
    #[error("the provisioned user credential could not be loaded: {reason}")]
    UserStore {
        /// Stable reason reported by the credential store.
        reason: String,
    },
    /// The installation token supplied on standard input was unusable.
    #[error("the installation token on standard input is unusable: {reason}")]
    StandardInput {
        /// Stable description of why the token was rejected.
        reason: &'static str,
    },
}

/// Loads a provisioned user credential from its store directory.
///
/// The composition root supplies the implementation so the encrypted store and
/// its `systemd-creds` encryptor stay a composition-root concern. Tests supply
/// a store backed by a reversible in-memory encryptor.
pub trait UserTokenLoader {
    /// Loads the stored user credential below `root`.
    ///
    /// # Errors
    ///
    /// Returns [`CredentialError::UserStore`] when the credential cannot be
    /// read or decrypted.
    fn load_user_token(&self, root: &Utf8Path) -> Result<AccessToken, CredentialError>;
}

/// Acquires the credential the mode names, before any request is attempted.
pub(super) fn acquire(
    mode: &ListingMode,
    loader: &dyn UserTokenLoader,
    stdin: &mut dyn Read,
) -> Result<GitHubCredential, CredentialError> {
    match mode {
        ListingMode::UserStore { root } => loader.load_user_token(root).map(GitHubCredential::User),
        ListingMode::InstallationStdin => {
            read_installation_token(stdin).map(GitHubCredential::Installation)
        }
    }
}

/// Reads one installation token from a bounded standard input.
pub(super) fn read_installation_token(
    stdin: &mut dyn Read,
) -> Result<InstallationAccessToken, CredentialError> {
    let bytes = read_bounded(stdin)?;
    let text = String::from_utf8(bytes).map_err(|_| invalid_stdin("not valid UTF-8"))?;
    InstallationAccessToken::new(strip_one_line_ending(&text)).map_err(|error| match error {
        RepositoryListingError::InvalidInput { reason, .. } => invalid_stdin(reason),
        _ => invalid_stdin("not a usable bearer value"),
    })
}

/// Reads at most the token limit plus one sentinel byte.
///
/// Reading the extra byte is what distinguishes "exactly at the limit" from
/// "over it" without buffering the rest of the input.
fn read_bounded(stdin: &mut dyn Read) -> Result<Vec<u8>, CredentialError> {
    let sentinel_limit = MAX_INSTALLATION_TOKEN_BYTES.saturating_add(1);
    let mut bytes = Vec::new();
    let mut limited = stdin.take(u64::try_from(sentinel_limit).unwrap_or(u64::MAX));
    limited
        .read_to_end(&mut bytes)
        .map_err(|_| invalid_stdin("could not be read"))?;
    if bytes.len() > MAX_INSTALLATION_TOKEN_BYTES {
        return Err(invalid_stdin("exceeds 16 KiB"));
    }
    Ok(bytes)
}

/// Removes at most one trailing line ending, leaving any other content intact.
fn strip_one_line_ending(text: &str) -> &str {
    text.strip_suffix("\r\n")
        .or_else(|| text.strip_suffix('\n'))
        .unwrap_or(text)
}

/// Builds a sanitized standard-input credential error.
const fn invalid_stdin(reason: &'static str) -> CredentialError {
    CredentialError::StandardInput { reason }
}
