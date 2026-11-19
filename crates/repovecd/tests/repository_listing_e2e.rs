//! End-to-end tests for the one-shot listing composition.
//!
//! Each test calls the same composition function the runnable example calls,
//! with a loopback endpoint and in-memory streams, so the evidence covers
//! credential acquisition, the real adapter, and output ordering together. No
//! test reaches GitHub, and every credential is a fake.

#[path = "common/listing_composition.rs"]
mod listing_composition;

use std::io;

use camino::Utf8PathBuf;
use listing_composition::{
    ByteBudgetWriter,
    EncryptedUserTokens,
    ReversibleEncryptor,
    UnusedUserTokens,
    run_buffered,
};
use repovec_core::github_oauth::AccessToken;
use repovec_test_helpers::{
    recording_http::{RecordingServer, ScriptedResponse},
    repository_listing_wire::{
        INSTALLATION_TOKEN,
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

#[test]
fn installation_mode_writes_every_observation_after_success() {
    let server = RecordingServer::start(vec![
        ScriptedResponse::new(200)
            .with_body(envelope_page(&[repository_json(7, "acme", "billing")]))
            .with_header(
                "Link",
                &link_header(&format!("{{origin}}{}", installation_page(2)), "next"),
            ),
        ScriptedResponse::new(200)
            .with_body(envelope_page(&[repository_json(8, "acme", "portal")])),
    ])
    .expect("the scripted server should start");
    let stdin = format!("{INSTALLATION_TOKEN}\n").into_bytes();

    let (status, stdout, stderr) = run_buffered(
        &server,
        ListingMode::InstallationStdin,
        &mut stdin.as_slice(),
        &UnusedUserTokens,
    )
    .expect("the composition fixture should run");

    assert_eq!(
        status,
        RunStatus::Success,
        "a completed traversal must succeed"
    );
    assert_eq!(
        stdout, "acme/billing\nacme/portal\n",
        "every observation must be written in page order"
    );
    assert!(
        stderr.is_empty(),
        "a successful run must write no diagnostic"
    );
    assert_eq!(
        server.request_count(),
        2,
        "both pages must be requested exactly once"
    );
    assert!(
        !stdout.contains(INSTALLATION_TOKEN) && !stderr.contains(INSTALLATION_TOKEN),
        "no stream may reveal the installation token"
    );
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
    let server = RecordingServer::start(vec![
        ScriptedResponse::new(200).with_body(bare_page(&[repository_json(
            1,
            "octocat",
            "hello-world",
        )])),
    ])
    .expect("the scripted server should start");
    let mut stdin = io::empty();

    let (status, stdout, stderr) = run_buffered(
        &server,
        ListingMode::UserStore { root },
        &mut stdin,
        &EncryptedUserTokens,
    )
    .expect("the composition fixture should run");

    assert_eq!(
        status,
        RunStatus::Success,
        "the stored credential must be usable"
    );
    assert_eq!(
        stdout, "octocat/hello-world\n",
        "the observation must be written once"
    );
    assert!(
        stderr.is_empty(),
        "a successful run must write no diagnostic"
    );
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

#[test]
fn empty_standard_input_fails_before_any_request() {
    let server = RecordingServer::start(Vec::new()).expect("the scripted server should start");
    let mut stdin = io::empty();

    let (status, stdout, stderr) = run_buffered(
        &server,
        ListingMode::InstallationStdin,
        &mut stdin,
        &UnusedUserTokens,
    )
    .expect("the composition fixture should run");

    assert_eq!(
        status,
        RunStatus::CredentialFailure,
        "an empty credential must fail before the traversal"
    );
    assert!(
        stdout.is_empty(),
        "a credential failure must write no observation"
    );
    assert!(
        stderr.contains("standard input"),
        "the diagnostic must name the credential source: {stderr}"
    );
    assert_eq!(
        server.request_count(),
        0,
        "no request may be attempted without a credential"
    );
}

#[test]
fn oversize_standard_input_fails_before_any_request() {
    let server = RecordingServer::start(Vec::new()).expect("the scripted server should start");
    let input = vec![b'a'; MAX_INSTALLATION_TOKEN_BYTES.saturating_add(1)];
    let mut stdin = input.as_slice();

    let (status, stdout, stderr) = run_buffered(
        &server,
        ListingMode::InstallationStdin,
        &mut stdin,
        &UnusedUserTokens,
    )
    .expect("the composition fixture should run");

    assert_eq!(
        status,
        RunStatus::CredentialFailure,
        "an oversize credential must fail before the traversal"
    );
    assert!(
        stdout.is_empty(),
        "a credential failure must write no observation"
    );
    assert!(
        stderr.contains("exceeds 16 KiB"),
        "the diagnostic must name the crossed limit: {stderr}"
    );
    assert_eq!(
        server.request_count(),
        0,
        "no request may be attempted with an oversize credential"
    );
}

#[test]
fn a_late_failure_writes_no_repository_output() {
    let server = RecordingServer::start(vec![
        ScriptedResponse::new(200)
            .with_body(envelope_page(&[repository_json(7, "acme", "billing")]))
            .with_header(
                "Link",
                &link_header(&format!("{{origin}}{}", installation_page(2)), "next"),
            ),
        ScriptedResponse::new(401),
    ])
    .expect("the scripted server should start");
    let stdin = format!("{INSTALLATION_TOKEN}\n").into_bytes();

    let (status, stdout, stderr) = run_buffered(
        &server,
        ListingMode::InstallationStdin,
        &mut stdin.as_slice(),
        &UnusedUserTokens,
    )
    .expect("the composition fixture should run");

    assert_eq!(
        status,
        RunStatus::ListingFailure,
        "a rejected later page must fail the whole listing"
    );
    assert!(
        stdout.is_empty(),
        "the accepted first page must not masquerade as a complete listing"
    );
    assert!(
        stderr.contains("authentication_rejected"),
        "the diagnostic must carry the stable category: {stderr}"
    );
    assert!(
        stderr.contains("fresh installation token"),
        "the diagnostic must state the installation recovery step: {stderr}"
    );
    assert!(
        !stderr.contains(INSTALLATION_TOKEN),
        "the diagnostic must not reveal the credential"
    );
}

#[test]
fn an_output_failure_keeps_the_lines_it_already_wrote() {
    let server = RecordingServer::start(vec![ScriptedResponse::new(200).with_body(envelope_page(
        &[
            repository_json(1, "octocat", "hello-world"),
            repository_json(2, "acme", "service"),
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

    let (status, stdout, stderr) = run_buffered(
        &server,
        ListingMode::UserStore { root },
        &mut stdin,
        &EncryptedUserTokens,
    )
    .expect("the composition fixture should run");

    assert_eq!(
        status,
        RunStatus::ListingFailure,
        "an unusable stored credential must fail adapter construction"
    );
    assert!(stdout.is_empty(), "no observation can precede a request");
    assert!(
        stderr.contains("invalid_input"),
        "the diagnostic must carry the stable category: {stderr}"
    );
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

    let (status, stdout, stderr) = run_buffered(
        &server,
        ListingMode::UserStore { root },
        &mut stdin,
        &EncryptedUserTokens,
    )
    .expect("the composition fixture should run");

    assert_eq!(
        status,
        RunStatus::CredentialFailure,
        "an unreadable store must fail credential acquisition"
    );
    assert!(
        stdout.is_empty(),
        "a credential failure must write no observation"
    );
    assert!(
        stderr.contains("could not be loaded"),
        "the diagnostic must report the acquisition failure: {stderr}"
    );
    assert_eq!(
        server.request_count(),
        0,
        "no request may be attempted without a credential"
    );
}
