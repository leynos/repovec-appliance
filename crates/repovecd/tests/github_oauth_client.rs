//! Integration tests for the GitHub OAuth HTTP adapter.

use std::{
    io::{Read, Write},
    net::TcpListener,
    thread,
};

use oauth2_test_server::OAuthTestServer;
use repovec_core::github_oauth::{ClientId, DeviceCode, TokenPollOutcome};
use repovecd::github_oauth_client::{DeviceFlowEndpoints, GitHubOAuthClient, OAuthClientError};
use serde_json::json;

#[test]
fn oauth2_test_server_exercises_device_code_request_and_token_poll() {
    let runtime = tokio::runtime::Runtime::new().expect("Tokio runtime should start");
    let server = runtime.block_on(OAuthTestServer::start());
    let registered_client = runtime.block_on(server.register_client(json!({
        "scope": "openid profile",
        "grant_types": ["urn:ietf:params:oauth:grant-type:device_code"],
        "client_name": "repovec-device-flow-integration-test"
    })));
    let oauth_client = GitHubOAuthClient::new(DeviceFlowEndpoints::new(
        server.endpoints.device_code.clone(),
        server.endpoints.device_token.clone(),
    ))
    .expect("OAuth client should be constructed");
    let client_id = ClientId::new(registered_client.client_id);

    let authorization = oauth_client
        .request_device_code(&client_id, ["openid", "profile"])
        .expect("device-code request should succeed");
    assert_eq!(authorization.interval.as_secs(), 5);
    assert_ne!(authorization.device_code.secret(), "");
    assert_ne!(authorization.user_code.as_str(), "");

    let pending = oauth_client
        .poll_token(&client_id, &authorization.device_code)
        .expect("unapproved device code should produce a polling outcome");
    assert_eq!(pending, TokenPollOutcome::AuthorizationPending);

    runtime.block_on(
        server.approve_device_code(authorization.device_code.secret(), "repovec-test-user"),
    );
    let authorized = oauth_client
        .poll_token(&client_id, &authorization.device_code)
        .expect("approved device code should return an access token");
    let TokenPollOutcome::Authorized(token) = authorized else {
        panic!("approved device code should authorize");
    };

    assert_ne!(token.secret(), "");
    assert!(token.scopes().iter().any(|scope| scope == "openid"));
    assert!(token.scopes().iter().any(|scope| scope == "profile"));
}

#[test]
fn malformed_device_code_json_is_reported() {
    let endpoint = malformed_json_endpoint().expect("test server should start");
    let oauth_client = GitHubOAuthClient::new(DeviceFlowEndpoints::new(&endpoint, &endpoint))
        .expect("OAuth client should be constructed");

    let result = oauth_client.request_device_code(&ClientId::new("client"), ["repo"]);

    assert!(matches!(result, Err(OAuthClientError::ReadDeviceCode(_))));
}

#[test]
fn malformed_token_json_is_reported() {
    let endpoint = malformed_json_endpoint().expect("test server should start");
    let oauth_client = GitHubOAuthClient::new(DeviceFlowEndpoints::new(&endpoint, &endpoint))
        .expect("OAuth client should be constructed");

    let result = oauth_client.poll_token(&ClientId::new("client"), &DeviceCode::new("device"));

    assert!(matches!(result, Err(OAuthClientError::ReadToken(_))));
}

fn malformed_json_endpoint() -> std::io::Result<String> {
    let listener = TcpListener::bind("127.0.0.1:0")?;
    let address = listener.local_addr()?;
    thread::spawn(move || serve_malformed_json(&listener));
    Ok(format!("http://{address}/oauth"))
}

/// Answer the single request the client under test sends with invalid JSON.
///
/// The helper returns no value because the server thread has no caller to
/// report to; a broken fixture is a test bug, so it panics there. Keeping it
/// out of [`malformed_json_endpoint`] keeps that fallible function free of
/// panics, which `clippy::panic_in_result_fn` requires.
fn serve_malformed_json(listener: &TcpListener) {
    let (mut stream, _) = match listener.accept() {
        Ok(accepted) => accepted,
        Err(error) => panic!("test server should accept one request: {error}"),
    };
    let mut request = [0_u8; 1024];
    let _bytes_read = match stream.read(&mut request) {
        Ok(bytes_read) => bytes_read,
        Err(error) => panic!("test server should read request: {error}"),
    };
    if let Err(error) = stream.write_all(
        b"HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: 1\r\n\r\n{",
    ) {
        panic!("test server should write response: {error}");
    }
}
