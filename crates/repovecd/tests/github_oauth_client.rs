//! Integration tests for the GitHub OAuth HTTP adapter.

use oauth2_test_server::OAuthTestServer;
use repovec_core::github_oauth::{ClientId, TokenPollOutcome};
use repovecd::github_oauth_client::{DeviceFlowEndpoints, GitHubOAuthClient};
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
    assert!(!authorization.device_code.secret().is_empty());
    assert!(!authorization.user_code.as_str().is_empty());

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

    assert!(!token.secret().is_empty());
    assert!(token.scopes().iter().any(|scope| scope == "openid"));
    assert!(token.scopes().iter().any(|scope| scope == "profile"));
}
