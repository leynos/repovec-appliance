//! Tests for encrypted GitHub token storage.

use std::{
    cell::RefCell,
    convert::Infallible,
    ffi::OsStr,
    path::PathBuf,
    rc::Rc,
    sync::{Arc, Barrier},
    thread,
};

#[cfg(unix)]
use cap_std::fs_utf8::PermissionsExt;
use cap_std::{ambient_authority, fs_utf8::Dir};
use repovec_core::github_oauth::AccessToken;
use rstest::{fixture, rstest};
use thiserror::Error;

use super::{
    CommandOutput, CommandRunner, CredentialEncryptor, EncryptedGitHubTokenStore,
    SystemdCredsEncryptor, SystemdCredsError, TokenStoreError,
};
use crate::tracing_test::capture_info_logs;

#[derive(Clone, Debug)]
struct PrefixEncryptor;

impl CredentialEncryptor for PrefixEncryptor {
    type Error = Infallible;

    fn encrypt(&self, plaintext: &[u8]) -> Result<Vec<u8>, Self::Error> {
        let mut encrypted = b"encrypted:".to_vec();
        encrypted.extend(plaintext.iter().rev());
        Ok(encrypted)
    }

    fn decrypt(&self, ciphertext: &[u8]) -> Result<Vec<u8>, Self::Error> {
        let encrypted = ciphertext.strip_prefix(b"encrypted:").unwrap_or(ciphertext);
        Ok(encrypted.iter().rev().copied().collect())
    }
}

struct StoredTokenFixture {
    _tempdir: tempfile::TempDir,
    root: camino::Utf8PathBuf,
    store: EncryptedGitHubTokenStore<PrefixEncryptor>,
}

#[derive(Debug, Error)]
enum StoredTokenFixtureError {
    #[error("tempdir should be created")]
    TempDir(#[from] std::io::Error),
    #[error("temporary path should be UTF-8: {0:?}")]
    NonUtf8Path(PathBuf),
    #[error("token store setup failed")]
    Store(#[from] TokenStoreError<Infallible>),
    #[error("temporary root should open")]
    OpenRoot(#[source] std::io::Error),
    #[error("encrypted credential should be readable")]
    ReadCredential(#[source] std::io::Error),
    #[error("encrypted credential metadata should be readable")]
    ReadMetadata(#[source] std::io::Error),
    #[error("stored credential was not encrypted")]
    CredentialWasNotEncrypted,
    #[error("stored credential exposed plaintext token material")]
    CredentialExposedPlaintext,
    #[error("credential mode should be 0600, got {0:o}")]
    WrongCredentialMode(u32),
    #[error("loaded token secret should match stored token, got {0}")]
    WrongLoadedToken(String),
}

#[derive(Clone, Debug)]
struct RecordingRunner {
    calls: Rc<RefCell<Vec<Vec<String>>>>,
    stdins: Rc<RefCell<Vec<Vec<u8>>>>,
}

impl RecordingRunner {
    fn new() -> Self {
        Self { calls: Rc::new(RefCell::new(Vec::new())), stdins: Rc::new(RefCell::new(Vec::new())) }
    }

    fn calls(&self) -> Vec<Vec<String>> { self.calls.borrow().clone() }

    fn stdins(&self) -> Vec<Vec<u8>> { self.stdins.borrow().clone() }
}

impl CommandRunner for RecordingRunner {
    type Error = Infallible;

    fn run<I, S>(
        &self,
        _command: &camino::Utf8Path,
        args: I,
        stdin: &[u8],
    ) -> Result<CommandOutput, Self::Error>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<OsStr>,
    {
        let collected = args
            .into_iter()
            .map(|arg| arg.as_ref().to_string_lossy().into_owned())
            .collect::<Vec<_>>();
        self.calls.borrow_mut().push(collected.clone());
        self.stdins.borrow_mut().push(stdin.to_vec());
        if collected.last().is_none_or(|output_path| output_path != "-") {
            return Ok(CommandOutput {
                status_code: Some(2),
                stderr: b"missing stdout marker".to_vec(),
                stdout: Vec::new(),
            });
        }
        Ok(CommandOutput {
            status_code: Some(0),
            stderr: Vec::new(),
            stdout: b"ciphertext".to_vec(),
        })
    }
}

#[rstest]
fn store_writes_only_encrypted_token_material(
    stored_token: Result<StoredTokenFixture, StoredTokenFixtureError>,
) -> Result<(), StoredTokenFixtureError> {
    let fixture = stored_token?;
    let readable_root = Dir::open_ambient_dir(&fixture.root, ambient_authority())
        .map_err(StoredTokenFixtureError::OpenRoot)?;
    let contents = readable_root
        .read(EncryptedGitHubTokenStore::<PrefixEncryptor>::credential_file())
        .map_err(StoredTokenFixtureError::ReadCredential)?;
    if !contents.starts_with(b"encrypted:") {
        return Err(StoredTokenFixtureError::CredentialWasNotEncrypted);
    }
    if String::from_utf8_lossy(&contents).contains("gho_secret") {
        return Err(StoredTokenFixtureError::CredentialExposedPlaintext);
    }
    Ok(())
}

#[cfg(unix)]
#[rstest]
fn store_writes_owner_only_credential_permissions(
    stored_token: Result<StoredTokenFixture, StoredTokenFixtureError>,
) -> Result<(), StoredTokenFixtureError> {
    let fixture = stored_token?;
    let readable_root = Dir::open_ambient_dir(&fixture.root, ambient_authority())
        .map_err(StoredTokenFixtureError::OpenRoot)?;
    let metadata = readable_root
        .metadata(EncryptedGitHubTokenStore::<PrefixEncryptor>::credential_file())
        .map_err(StoredTokenFixtureError::ReadMetadata)?;
    let mode = metadata.permissions().mode() & 0o777;
    if mode != 0o600 {
        return Err(StoredTokenFixtureError::WrongCredentialMode(mode));
    }
    Ok(())
}

#[rstest]
fn load_decrypts_the_stored_token(
    stored_token: Result<StoredTokenFixture, StoredTokenFixtureError>,
) -> Result<(), StoredTokenFixtureError> {
    let fixture = stored_token?;
    let token = fixture.store.load_token()?;
    if token.secret() != "gho_secret" {
        return Err(StoredTokenFixtureError::WrongLoadedToken(token.secret().to_owned()));
    }
    Ok(())
}

#[test]
fn token_store_write_metrics_are_emitted_as_events() {
    let ((), logs) = capture_info_logs(|| {
        super::observability::info_token_store_write(std::time::Instant::now());
    })
    .expect("capturing tracing logs should succeed");

    assert!(logs.contains("metric.github_token_store_write_duration_ms"));
    assert!(logs.contains("elapsed_ms="));
    assert!(logs.contains("metric.github_token_store_write_total"));
}

#[test]
fn token_store_load_metrics_are_emitted_as_events() {
    let ((), logs) = capture_info_logs(|| {
        super::observability::info_token_store_load(std::time::Instant::now());
    })
    .expect("capturing tracing logs should succeed");

    assert!(logs.contains("metric.github_token_store_load_duration_ms"));
    assert!(logs.contains("elapsed_ms="));
    assert!(logs.contains("metric.github_token_store_load_total"));
}

#[fixture]
fn stored_token() -> Result<StoredTokenFixture, StoredTokenFixtureError> {
    let stored_token = build_stored_token()?;
    Ok(stored_token)
}

fn build_stored_token() -> Result<StoredTokenFixture, StoredTokenFixtureError> {
    let tempdir = tempfile::tempdir()?;
    let root = camino::Utf8PathBuf::from_path_buf(tempdir.path().to_path_buf())
        .map_err(StoredTokenFixtureError::NonUtf8Path)?;
    let store = EncryptedGitHubTokenStore::open(&root, PrefixEncryptor)?;

    store.store_token(&AccessToken::new("gho_secret", ["repo"]))?;

    Ok(StoredTokenFixture { _tempdir: tempdir, root, store })
}

#[test]
fn systemd_creds_encryptor_does_not_pass_token_as_an_argument() {
    let runner = RecordingRunner::new();
    let encryptor = SystemdCredsEncryptor::new(runner.clone());

    let encrypted = encryptor.encrypt(b"gho_secret").expect("token should encrypt");

    assert_eq!(encrypted, b"ciphertext");
    assert!(runner.calls().iter().flatten().all(|arg| !arg.contains("gho_secret")));
    assert_eq!(runner.stdins(), [b"gho_secret".to_vec()]);
}

#[test]
fn systemd_creds_encryptor_reads_input_from_stdin() {
    let runner = RecordingRunner::new();
    let encryptor = SystemdCredsEncryptor::new(runner.clone());

    encryptor.encrypt(b"gho_secret").expect("token should encrypt");

    assert_eq!(
        runner.calls(),
        [vec![
            "encrypt".to_owned(),
            "--name=repovec-github-oauth-token".to_owned(),
            "-".to_owned(),
            "-".to_owned(),
        ]]
    );
}

#[test]
fn systemd_creds_decrypt_uses_the_credential_name_binding() {
    let runner = RecordingRunner::new();
    let encryptor = SystemdCredsEncryptor::new(runner.clone());

    let decrypted = encryptor.decrypt(b"ciphertext").expect("token should decrypt");

    assert_eq!(decrypted, b"ciphertext");
    assert!(
        runner
            .calls()
            .iter()
            .any(|args| args.iter().any(|arg| arg == "--name=repovec-github-oauth-token"))
    );
}

#[test]
fn systemd_creds_error_display_redacts_token() {
    let error = SystemdCredsError::<Infallible>::CommandFailed {
        operation: "encrypt",
        status: Some(1),
        stderr: super::LossyStderr("failure for gho_secret".to_owned()),
    };
    let message = error.to_string();

    assert!(!message.contains("gho_secret"));
    assert!(message.contains("[redacted]"));
    insta::assert_snapshot!(
        message,
        @"systemd-creds encrypt failed with status Some(1): failure for [redacted]"
    );
}

#[test]
fn systemd_creds_error_debug_redacts_token() {
    let error = SystemdCredsError::<Infallible>::CommandFailed {
        operation: "encrypt",
        status: Some(1),
        stderr: super::LossyStderr("failure for gho_secret".to_owned()),
    };
    let message = format!("{error:?}");

    assert!(!message.contains("gho_secret"));
    assert!(message.contains("[redacted]"));
    insta::assert_snapshot!(
        message,
        @"CommandFailed { operation: \"encrypt\", status: Some(1), stderr: LossyStderr(\"failure for [redacted]\") }"
    );
}

#[rstest]
#[case("gho_secret")]
#[case("ghp_secret")]
#[case("ghs_secret")]
#[case("ghr_secret")]
#[case("ghu_secret")]
#[case("github_pat_secret")]
fn systemd_creds_error_display_redacts_github_token_prefixes(#[case] token: &str) {
    let error = SystemdCredsError::<Infallible>::CommandFailed {
        operation: "encrypt",
        status: Some(1),
        stderr: super::LossyStderr(format!("failure for {token}")),
    };
    let message = error.to_string();

    assert!(!message.contains(token));
    assert!(message.contains("[redacted]"));
}

#[test]
fn atomic_writes_use_unique_temporary_files_under_concurrency() {
    let tempdir = tempfile::tempdir().expect("tempdir should be created");
    let root_path = camino::Utf8PathBuf::from_path_buf(tempdir.path().to_path_buf())
        .expect("temporary path should be UTF-8");
    let start_barrier = Arc::new(Barrier::new(8));
    let handles = (0..8)
        .map(|index| {
            let writer_root_path = root_path.clone();
            let writer_start_barrier = Arc::clone(&start_barrier);
            thread::spawn(move || {
                let opened_root = Dir::open_ambient_dir(&writer_root_path, ambient_authority())
                    .expect("temporary root should open");
                writer_start_barrier.wait();
                let contents = format!("ciphertext-{index}");
                super::write_atomically(
                    &opened_root,
                    EncryptedGitHubTokenStore::<PrefixEncryptor>::credential_file(),
                    contents.as_bytes(),
                )
                .expect("atomic write should succeed");
            })
        })
        .collect::<Vec<_>>();

    for handle in handles {
        handle.join().expect("writer thread should not panic");
    }

    let readable_root =
        Dir::open_ambient_dir(&root_path, ambient_authority()).expect("temporary root should open");
    let raw_contents = readable_root
        .read(EncryptedGitHubTokenStore::<PrefixEncryptor>::credential_file())
        .expect("encrypted credential should be readable");
    let contents = String::from_utf8(raw_contents).expect("test ciphertext should be UTF-8");

    assert!((0..8).any(|index| contents == format!("ciphertext-{index}")));
}
