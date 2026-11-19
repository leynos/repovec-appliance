//! End-to-end tests for the one-shot listing composition.
//!
//! Each test calls the same composition function the runnable example calls,
//! with a loopback endpoint and in-memory streams, so the evidence covers
//! credential acquisition, the real adapter, and output ordering together. No
//! test reaches GitHub, and every credential is a fake.

#[path = "common/listing_composition.rs"]
mod listing_composition;
#[path = "common/listing_observation.rs"]
mod listing_observation;

use std::io;

use camino::Utf8PathBuf;
use listing_composition::{
    ByteBudgetWriter,
    EncryptedUserTokens,
    ReversibleEncryptor,
    UnusedUserTokens,
};
use listing_observation::observe;
use repovec_core::github_oauth::AccessToken;
use repovec_test_helpers::{
    recording_http::{RecordingServer, ScriptedResponse},
    repository_listing_wire::{
        INSTALLATION_TOKEN,
        LinkRelation,
        Repository,
        USER_TOKEN,
        bare_page,
        envelope_page,
        installation_page,
        link_header,
        repository_json,
    },
};
use repovecd::{
    github_repository_client::{GitHubApiEndpoint, ListingLimits},
    github_token_store::EncryptedGitHubTokenStore,
    repository_listing_command::{
        ListingMode,
        ListingRun,
        MAX_INSTALLATION_TOKEN_BYTES,
        RunStatus,
        run_listing,
    },
};
use rstest::rstest;

#[test]
fn installation_mode_writes_every_observation_after_success() {
    let server = RecordingServer::start(vec![
        ScriptedResponse::new(200)
            .with_body(envelope_page(&[repository_json(Repository {
                id: 7,
                owner: "acme",
                name: "billing",
            })]))
            .with_header(
                "Link",
                &link_header(
                    &format!("{{origin}}{}", installation_page(2)),
                    LinkRelation::Next,
                ),
            ),
        ScriptedResponse::new(200).with_body(envelope_page(&[repository_json(Repository {
            id: 8,
            owner: "acme",
            name: "portal",
        })])),
    ])
    .expect("the scripted server should start");
    let stdin = format!("{INSTALLATION_TOKEN}\n").into_bytes();

    let observed = observe(
        &server,
        ListingMode::InstallationStdin,
        &mut stdin.as_slice(),
        &UnusedUserTokens,
    )
    .expect("the composition fixture should run");

    observed.assert_succeeded("acme/billing\nacme/portal\n");
    assert_eq!(
        server.request_count(),
        2,
        "both pages must be requested exactly once"
    );
    observed.assert_hides(INSTALLATION_TOKEN);
}

#[test]
fn user_store_mode_loads_the_encrypted_credential() {
    let directory = tempfile::tempdir().expect("a temporary directory should be created");
    let root = Utf8PathBuf::from_path_buf(directory.path().to_path_buf())
        .expect("the temporary path is UTF-8");
    let store = EncryptedGitHubTokenStore::open(&root, ReversibleEncryptor)
        .expect("the token store should open");
    store
        .store_token(&AccessToken::new(USER_TOKEN, ["repo"]))
        .expect("the token should be stored");
    let server = RecordingServer::start(vec![ScriptedResponse::new(200).with_body(bare_page(&[
        repository_json(Repository {
            id: 1,
            owner: "octocat",
            name: "hello-world",
        }),
    ]))])
    .expect("the scripted server should start");
    let mut stdin = io::empty();

    let observed = observe(
        &server,
        ListingMode::UserStore { root },
        &mut stdin,
        &EncryptedUserTokens,
    )
    .expect("the composition fixture should run");

    observed.assert_succeeded("octocat/hello-world\n");
    let requests = server.requests();
    let request = requests
        .first()
        .expect("the page request should be recorded");
    assert_eq!(
        request.header("authorization"),
        Some(format!("Bearer {USER_TOKEN}").as_str()),
        "the credential loaded from the store must be presented"
    );
    assert_eq!(
        request.path(),
        "/user/repos",
        "the user mode must page the documented endpoint"
    );
}

/// A standard-input payload an installation-mode run must refuse.
#[derive(Clone, Copy, Debug)]
enum RefusedInput {
    /// No bytes at all.
    Empty,
    /// One byte past the ceiling, so the token line is not even well formed.
    Oversize,
}

impl RefusedInput {
    /// Returns the bytes the run reads.
    fn bytes(self) -> Vec<u8> {
        match self {
            Self::Empty => Vec::new(),
            Self::Oversize => vec![b'a'; MAX_INSTALLATION_TOKEN_BYTES.saturating_add(1)],
        }
    }

    /// Returns the text the diagnostic must contain for this payload.
    const fn cause(self) -> &'static str {
        match self {
            Self::Empty => "standard input",
            Self::Oversize => "exceeds 16 KiB",
        }
    }
}

#[rstest]
#[case::empty(RefusedInput::Empty)]
#[case::oversize(RefusedInput::Oversize)]
fn installation_input_outside_the_contract_is_refused(#[case] input: RefusedInput) {
    let server = RecordingServer::start(Vec::new()).expect("the scripted server should start");
    let stdin = input.bytes();

    let observed = observe(
        &server,
        ListingMode::InstallationStdin,
        &mut stdin.as_slice(),
        &UnusedUserTokens,
    )
    .expect("the composition fixture should run");

    observed.assert_refused(RunStatus::CredentialFailure, input.cause());
    assert_eq!(
        server.request_count(),
        0,
        "no request may be attempted for {input:?}"
    );
}

#[test]
fn a_late_failure_writes_no_repository_output() {
    let server = RecordingServer::start(vec![
        ScriptedResponse::new(200)
            .with_body(envelope_page(&[repository_json(Repository {
                id: 7,
                owner: "acme",
                name: "billing",
            })]))
            .with_header(
                "Link",
                &link_header(
                    &format!("{{origin}}{}", installation_page(2)),
                    LinkRelation::Next,
                ),
            ),
        ScriptedResponse::new(401),
    ])
    .expect("the scripted server should start");
    let stdin = format!("{INSTALLATION_TOKEN}\n").into_bytes();

    let observed = observe(
        &server,
        ListingMode::InstallationStdin,
        &mut stdin.as_slice(),
        &UnusedUserTokens,
    )
    .expect("the composition fixture should run");

    observed.assert_refused(RunStatus::ListingFailure, "authentication_rejected");
    assert!(
        observed.stderr.contains("fresh installation token"),
        "the diagnostic must state the installation recovery step: {}",
        observed.stderr
    );
    observed.assert_hides(INSTALLATION_TOKEN);
}

#[test]
fn an_output_failure_keeps_the_lines_it_already_wrote() {
    let server = RecordingServer::start(vec![ScriptedResponse::new(200).with_body(envelope_page(
        &[
            repository_json(Repository {
                id: 1,
                owner: "octocat",
                name: "hello-world",
            }),
            repository_json(Repository {
                id: 2,
                owner: "acme",
                name: "service",
            }),
        ],
    ))])
    .expect("the scripted server should start");
    let stdin = format!("{INSTALLATION_TOKEN}\n").into_bytes();
    let mut stdout = ByteBudgetWriter::new("octocat/hello-world\n".len());
    let mut stderr = Vec::new();

    let mut run = ListingRun {
        mode: ListingMode::InstallationStdin,
        endpoint: GitHubApiEndpoint::new(server.origin())
            .expect("a loopback origin should be accepted"),
        limits: ListingLimits::defaults(),
        load_user_token: &UnusedUserTokens,
        stdin: &mut stdin.as_slice(),
        stdout: &mut stdout,
        stderr: &mut stderr,
    };

    let status = run_listing(&mut run);
    assert_eq!(
        status,
        RunStatus::OutputFailure,
        "a writer that fails mid-listing must be reported"
    );
    assert_eq!(
        String::from_utf8(stdout.written).expect("the accepted bytes are ASCII"),
        "octocat/hello-world\n",
        "the line written before the failure stays visible"
    );
    let diagnostic = String::from_utf8(stderr).expect("the diagnostic is ASCII");
    assert!(
        diagnostic.contains("could not be written"),
        "the diagnostic must report the failed write: {diagnostic}"
    );
}

#[test]
fn a_whitespace_user_credential_fails_before_any_request() {
    let directory = tempfile::tempdir().expect("a temporary directory should be created");
    let root = Utf8PathBuf::from_path_buf(directory.path().to_path_buf())
        .expect("the temporary path is UTF-8");
    let store = EncryptedGitHubTokenStore::open(&root, ReversibleEncryptor)
        .expect("the token store should open");
    store
        .store_token(&AccessToken::new("gho_bad token", ["repo"]))
        .expect("the store persists any bearer value it is given");
    let server = RecordingServer::start(Vec::new()).expect("the scripted server should start");
    let mut stdin = io::empty();

    let observed = observe(
        &server,
        ListingMode::UserStore { root },
        &mut stdin,
        &EncryptedUserTokens,
    )
    .expect("the composition fixture should run");

    observed.assert_refused(RunStatus::ListingFailure, "invalid_input");
    assert_eq!(
        server.request_count(),
        0,
        "a credential that cannot be a header value must never be sent"
    );
}

#[test]
fn an_unreadable_user_store_fails_before_any_request() {
    let directory = tempfile::tempdir().expect("a temporary directory should be created");
    let root = Utf8PathBuf::from_path_buf(directory.path().join("absent"))
        .expect("the temporary path is UTF-8");
    let server = RecordingServer::start(Vec::new()).expect("the scripted server should start");
    let mut stdin = io::empty();

    let observed = observe(
        &server,
        ListingMode::UserStore { root },
        &mut stdin,
        &EncryptedUserTokens,
    )
    .expect("the composition fixture should run");

    observed.assert_refused(RunStatus::CredentialFailure, "could not be loaded");
    assert_eq!(
        server.request_count(),
        0,
        "no request may be attempted without a credential"
    );
}
