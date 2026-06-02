//! Tests for encrypted GitHub token storage.

use std::{cell::RefCell, convert::Infallible, ffi::OsStr, rc::Rc};

#[cfg(unix)]
use cap_std::fs_utf8::PermissionsExt;
use cap_std::{ambient_authority, fs_utf8::Dir};
use repovec_core::github_oauth::AccessToken;
use rstest::rstest;

use super::{
    CommandOutput, CommandRunner, CredentialEncryptor, EncryptedGitHubTokenStore,
    SystemdCredsEncryptor, SystemdCredsError,
};

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

#[derive(Clone, Debug)]
struct RecordingRunner {
    calls: Rc<RefCell<Vec<Vec<String>>>>,
}

impl RecordingRunner {
    fn new() -> Self { Self { calls: Rc::new(RefCell::new(Vec::new())) } }

    fn calls(&self) -> Vec<Vec<String>> { self.calls.borrow().clone() }
}

impl CommandRunner for RecordingRunner {
    type Error = Infallible;

    fn run<I, S>(&self, _command: &camino::Utf8Path, args: I) -> Result<CommandOutput, Self::Error>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<OsStr>,
    {
        let collected = args
            .into_iter()
            .map(|arg| arg.as_ref().to_string_lossy().into_owned())
            .collect::<Vec<_>>();
        self.calls.borrow_mut().push(collected.clone());
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

#[test]
fn store_writes_only_encrypted_token_material() {
    let tempdir = tempfile::tempdir().expect("tempdir should be created");
    let root = camino::Utf8Path::from_path(tempdir.path()).expect("temporary path should be UTF-8");
    let store = EncryptedGitHubTokenStore::open(root, PrefixEncryptor).expect("store should open");

    store.store_token(&AccessToken::new("gho_secret", ["repo"])).expect("token should be stored");

    let readable_root =
        Dir::open_ambient_dir(root, ambient_authority()).expect("temporary root should open");
    let contents = readable_root
        .read(EncryptedGitHubTokenStore::<PrefixEncryptor>::credential_file())
        .expect("encrypted credential should be readable");
    assert!(contents.starts_with(b"encrypted:"));
    assert!(!String::from_utf8_lossy(&contents).contains("gho_secret"));
}

#[cfg(unix)]
#[test]
fn store_writes_owner_only_credential_permissions() {
    let tempdir = tempfile::tempdir().expect("tempdir should be created");
    let root = camino::Utf8Path::from_path(tempdir.path()).expect("temporary path should be UTF-8");
    let store = EncryptedGitHubTokenStore::open(root, PrefixEncryptor).expect("store should open");

    store.store_token(&AccessToken::new("gho_secret", ["repo"])).expect("token should be stored");

    let readable_root =
        Dir::open_ambient_dir(root, ambient_authority()).expect("temporary root should open");
    let metadata = readable_root
        .metadata(EncryptedGitHubTokenStore::<PrefixEncryptor>::credential_file())
        .expect("encrypted credential metadata should be readable");
    assert_eq!(metadata.permissions().mode() & 0o777, 0o600);
}

#[test]
fn load_decrypts_the_stored_token() {
    let tempdir = tempfile::tempdir().expect("tempdir should be created");
    let root = camino::Utf8Path::from_path(tempdir.path()).expect("temporary path should be UTF-8");
    let store = EncryptedGitHubTokenStore::open(root, PrefixEncryptor).expect("store should open");

    store.store_token(&AccessToken::new("gho_secret", ["repo"])).expect("token should be stored");

    assert_eq!(store.load_token().expect("token should load").secret(), "gho_secret");
}

#[test]
fn systemd_creds_encryptor_does_not_pass_token_as_an_argument() {
    let runner = RecordingRunner::new();
    let encryptor = SystemdCredsEncryptor::new(runner.clone());

    let encrypted = encryptor.encrypt(b"gho_secret").expect("token should encrypt");

    assert_eq!(encrypted, b"ciphertext");
    assert!(runner.calls().iter().flatten().all(|arg| !arg.contains("gho_secret")));
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
