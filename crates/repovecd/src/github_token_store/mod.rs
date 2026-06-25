//! Encrypted storage adapter for GitHub OAuth access tokens.

use std::{
    ffi::OsStr,
    io::{ErrorKind, Write},
    process::{Command, Stdio},
    sync::atomic::{AtomicU64, Ordering},
    time::Instant,
};

use camino::{Utf8Path, Utf8PathBuf};
#[cfg(unix)]
use cap_std::fs_utf8::OpenOptionsExt;
use cap_std::{
    ambient_authority,
    fs_utf8::{Dir, OpenOptions},
};
use repovec_core::github_oauth::AccessToken;
use thiserror::Error;
use tracing::info_span;

use crate::github_device_flow::TokenStore;

mod redaction;

pub use redaction::LossyStderr;

const TOKEN_CREDENTIAL_NAME: &str = "repovec-github-oauth-token";
const TOKEN_CREDENTIAL_FILE: &str = "github-oauth-token.cred";
const ATOMIC_WRITE_CREATE_RETRIES: u8 = 8;

static NEXT_TEMP_FILE_ID: AtomicU64 = AtomicU64::new(0);

/// Encrypts and decrypts token material before filesystem persistence.
pub trait CredentialEncryptor {
    /// Error returned by the encryption adapter.
    type Error: std::error::Error + Send + Sync + 'static;

    /// Encrypts plaintext token bytes.
    ///
    /// # Errors
    ///
    /// Returns the adapter error when encryption fails.
    fn encrypt(&self, plaintext: &[u8]) -> Result<Vec<u8>, Self::Error>;

    /// Decrypts ciphertext token bytes.
    ///
    /// # Errors
    ///
    /// Returns the adapter error when decryption fails.
    fn decrypt(&self, ciphertext: &[u8]) -> Result<Vec<u8>, Self::Error>;
}

/// Encrypted GitHub token store rooted below `/etc/repovec`.
#[derive(Debug)]
pub struct EncryptedGitHubTokenStore<E> {
    root: Dir,
    encryptor: E,
}

impl<E> EncryptedGitHubTokenStore<E>
where
    E: CredentialEncryptor,
{
    /// Opens a token store rooted at the supplied directory.
    ///
    /// # Errors
    ///
    /// Returns an error if the directory cannot be opened.
    pub fn open(root: &Utf8Path, encryptor: E) -> Result<Self, TokenStoreError<E::Error>> {
        Dir::open_ambient_dir(root, ambient_authority())
            .map(|opened_root| Self { root: opened_root, encryptor })
            .map_err(TokenStoreError::OpenRoot)
    }

    /// Stores a token encrypted at rest.
    ///
    /// # Errors
    ///
    /// Returns an error if encryption or atomic persistence fails.
    pub fn store_token(&self, token: &AccessToken) -> Result<(), TokenStoreError<E::Error>> {
        let span = info_span!("github_token_store.store");
        let _entered = span.enter();
        let started_at = Instant::now();
        let ciphertext =
            self.encryptor.encrypt(token.secret().as_bytes()).map_err(TokenStoreError::Encrypt)?;
        write_atomically(&self.root, TOKEN_CREDENTIAL_FILE, &ciphertext)
            .map_err(TokenStoreError::Write)?;
        info_token_store_write(started_at);
        Ok(())
    }

    /// Loads and decrypts a token from disk.
    ///
    /// Reloaded tokens contain the persisted bearer secret only. Scope
    /// information is not persisted because GitHub authorisation scope is
    /// discovered during the live login response and should be revalidated by
    /// callers that need it after process restart.
    ///
    /// # Errors
    ///
    /// Returns an error if the file cannot be read, decrypted, or decoded as
    /// UTF-8.
    pub fn load_token(&self) -> Result<AccessToken, TokenStoreError<E::Error>> {
        let span = info_span!("github_token_store.load");
        let _entered = span.enter();
        let started_at = Instant::now();
        let ciphertext = self.root.read(TOKEN_CREDENTIAL_FILE).map_err(TokenStoreError::Read)?;
        let plaintext = self.encryptor.decrypt(&ciphertext).map_err(TokenStoreError::Decrypt)?;
        let token = String::from_utf8(plaintext).map_err(TokenStoreError::Decode)?;
        info_token_store_load(started_at);
        Ok(AccessToken::new(token, std::iter::empty::<String>()))
    }

    /// Returns the relative credential filename.
    #[must_use]
    pub const fn credential_file() -> &'static str { TOKEN_CREDENTIAL_FILE }
}

impl<E> TokenStore for EncryptedGitHubTokenStore<E>
where
    E: CredentialEncryptor,
{
    type Error = TokenStoreError<E::Error>;

    fn store(&self, token: &AccessToken) -> Result<(), Self::Error> { self.store_token(token) }
}

/// Runs `systemd-creds` for credential encryption and decryption.
#[derive(Clone, Debug)]
pub struct SystemdCredsEncryptor<R> {
    runner: R,
    command: Utf8PathBuf,
}

impl<R> SystemdCredsEncryptor<R>
where
    R: CommandRunner,
{
    /// Creates an encryptor using the supplied command runner.
    #[must_use]
    pub fn new(runner: R) -> Self { Self { runner, command: Utf8PathBuf::from("systemd-creds") } }

    /// Creates an encryptor with an explicit `systemd-creds` path.
    #[must_use]
    pub const fn with_command(runner: R, command: Utf8PathBuf) -> Self { Self { runner, command } }
}

impl<R> CredentialEncryptor for SystemdCredsEncryptor<R>
where
    R: CommandRunner,
{
    type Error = SystemdCredsError<R::Error>;

    fn encrypt(&self, plaintext: &[u8]) -> Result<Vec<u8>, Self::Error> {
        run_systemd_creds_file_transform(&SystemdCredsTransform {
            runner: &self.runner,
            command: &self.command,
            operation: "encrypt",
            name: Some(TOKEN_CREDENTIAL_NAME),
            input: plaintext,
        })
    }

    fn decrypt(&self, ciphertext: &[u8]) -> Result<Vec<u8>, Self::Error> {
        run_systemd_creds_file_transform(&SystemdCredsTransform {
            runner: &self.runner,
            command: &self.command,
            operation: "decrypt",
            name: Some(TOKEN_CREDENTIAL_NAME),
            input: ciphertext,
        })
    }
}

/// Command runner boundary for invoking `systemd-creds`.
pub trait CommandRunner {
    /// Error returned by the runner.
    type Error: std::error::Error + Send + Sync + 'static;

    /// Runs a command and returns its output bytes.
    ///
    /// # Errors
    ///
    /// Returns the runner error when the process cannot be started, written to,
    /// or waited on.
    fn run<I, S>(
        &self,
        command: &Utf8Path,
        args: I,
        stdin: &[u8],
    ) -> Result<CommandOutput, Self::Error>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<OsStr>;
}

/// Output returned by a command runner.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CommandOutput {
    /// Process exit code, or `None` if it was terminated by signal.
    pub status_code: Option<i32>,
    /// Captured standard error.
    pub stderr: Vec<u8>,
    /// Captured standard output.
    pub stdout: Vec<u8>,
}

/// Production command runner backed by `std::process::Command`.
#[derive(Clone, Copy, Debug, Default)]
pub struct SystemCommandRunner;

impl CommandRunner for SystemCommandRunner {
    type Error = std::io::Error;

    fn run<I, S>(
        &self,
        command: &Utf8Path,
        args: I,
        stdin: &[u8],
    ) -> Result<CommandOutput, Self::Error>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<OsStr>,
    {
        let mut child = Command::new(command.as_str())
            .args(args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()?;
        if let Some(mut child_stdin) = child.stdin.take()
            && let Err(error) = child_stdin.write_all(stdin)
        {
            drop(child.kill());
            drop(child.wait());
            return Err(error);
        }
        let output = child.wait_with_output()?;
        Ok(CommandOutput {
            status_code: output.status.code(),
            stderr: output.stderr,
            stdout: output.stdout,
        })
    }
}

/// Errors returned by encrypted token persistence.
#[derive(Debug, Error)]
pub enum TokenStoreError<E>
where
    E: std::error::Error + Send + Sync + 'static,
{
    /// The token store root could not be opened.
    #[error("failed to open GitHub token store root")]
    OpenRoot(#[source] std::io::Error),
    /// The token could not be encrypted.
    #[error("failed to encrypt GitHub OAuth token")]
    Encrypt(#[source] E),
    /// The encrypted token could not be written.
    #[error("failed to write encrypted GitHub OAuth token")]
    Write(#[source] std::io::Error),
    /// The encrypted token could not be read.
    #[error("failed to read encrypted GitHub OAuth token")]
    Read(#[source] std::io::Error),
    /// The encrypted token could not be decrypted.
    #[error("failed to decrypt GitHub OAuth token")]
    Decrypt(#[source] E),
    /// The decrypted token was not UTF-8.
    #[error("decrypted GitHub OAuth token was not UTF-8")]
    Decode(#[source] std::string::FromUtf8Error),
}

/// Errors returned by the `systemd-creds` encryptor.
#[derive(Debug, Error)]
pub enum SystemdCredsError<E>
where
    E: std::error::Error + Send + Sync + 'static,
{
    /// The `systemd-creds` command failed to run.
    #[error("failed to run systemd-creds")]
    Run(#[source] E),
    /// The `systemd-creds` command returned a non-zero exit status.
    #[error("systemd-creds {operation} failed with status {status:?}: {stderr}")]
    CommandFailed {
        /// Operation name.
        operation: &'static str,
        /// Process exit status code.
        status: Option<i32>,
        /// Captured standard error.
        stderr: LossyStderr,
    },
}

fn write_atomically(root: &Dir, filename: &str, contents: &[u8]) -> std::io::Result<()> {
    let (temp_name, mut temp_file) = create_atomic_write_temp_file(root, filename)?;
    temp_file.write_all(contents)?;
    temp_file.flush()?;
    temp_file.sync_all()?;
    drop(temp_file);
    root.rename(&temp_name, root, filename)?;
    root.open(".")?.sync_all()
}

fn create_atomic_write_temp_file(
    root: &Dir,
    filename: &str,
) -> std::io::Result<(String, cap_std::fs_utf8::File)> {
    for _ in 0..ATOMIC_WRITE_CREATE_RETRIES {
        let temp_name = next_atomic_write_temp_name(filename);
        let mut options = OpenOptions::new();
        options.create_new(true).write(true);
        #[cfg(unix)]
        options.mode(0o600);
        match root.open_with(&temp_name, &options) {
            Ok(file) => return Ok((temp_name, file)),
            Err(error) if error.kind() == ErrorKind::AlreadyExists => {}
            Err(error) => return Err(error),
        }
    }
    Err(std::io::Error::new(
        ErrorKind::AlreadyExists,
        "could not create a unique atomic-write temporary file",
    ))
}

fn next_atomic_write_temp_name(filename: &str) -> String {
    let id = NEXT_TEMP_FILE_ID.fetch_add(1, Ordering::Relaxed);
    format!(".{filename}.{}.{id}.tmp", std::process::id())
}

fn info_token_store_write(started_at: Instant) {
    let duration_span = info_span!(
        "metric.github_token_store_write_duration_ms",
        elapsed_ms = started_at.elapsed().as_millis(),
    );
    let _duration_entered = duration_span.enter();
    let total_span = info_span!("metric.github_token_store_write_total");
    let _total_entered = total_span.enter();
}

fn info_token_store_load(started_at: Instant) {
    let duration_span = info_span!(
        "metric.github_token_store_load_duration_ms",
        elapsed_ms = started_at.elapsed().as_millis(),
    );
    let _duration_entered = duration_span.enter();
    let total_span = info_span!("metric.github_token_store_load_total");
    let _total_entered = total_span.enter();
}

struct SystemdCredsTransform<'a, R>
where
    R: CommandRunner,
{
    runner: &'a R,
    command: &'a Utf8Path,
    operation: &'static str,
    name: Option<&'a str>,
    input: &'a [u8],
}

fn run_systemd_creds_file_transform<R>(
    transform: &SystemdCredsTransform<'_, R>,
) -> Result<Vec<u8>, SystemdCredsError<R::Error>>
where
    R: CommandRunner,
{
    let mut args = vec![transform.operation.to_owned()];
    if let Some(credential_name) = transform.name {
        args.push(format!("--name={credential_name}"));
    }
    args.push("-".to_owned());
    args.push("-".to_owned());

    let output = transform
        .runner
        .run(transform.command, args, transform.input)
        .map_err(SystemdCredsError::Run)?;
    if output.status_code != Some(0) {
        return Err(SystemdCredsError::CommandFailed {
            operation: transform.operation,
            status: output.status_code,
            stderr: LossyStderr(String::from_utf8_lossy(&output.stderr).into_owned()),
        });
    }

    Ok(output.stdout)
}

#[cfg(test)]
mod tests;
