//! Unit tests for OAuth adapter helpers.

use oauth2::reqwest::StatusCode;
use repovec_core::github_oauth::TokenPollOutcome;

use super::{
    DeviceFlowEndpoints, GitHubOAuthClient, OAuthClientError, TokenPollWire, join_scopes,
    token_poll_outcome,
};

#[test]
fn scopes_are_joined_with_spaces() {
    assert_eq!(join_scopes(["repo", "read:org"]), "repo read:org");
}

#[test]
fn invalid_device_code_url_is_rejected() {
    let result =
        GitHubOAuthClient::new(DeviceFlowEndpoints::new("not a URL", "https://example.test/token"));

    assert!(matches!(result, Err(OAuthClientError::InvalidDeviceCodeUrl(_))));
}

#[test]
fn invalid_token_url_is_rejected() {
    let result = GitHubOAuthClient::new(DeviceFlowEndpoints::new(
        "https://example.test/device",
        "not a URL",
    ));

    assert!(matches!(result, Err(OAuthClientError::InvalidTokenUrl(_))));
}

#[test]
fn token_poll_maps_device_flow_error_codes() {
    let pending = token_poll_outcome(
        TokenPollWire {
            access_token: None,
            error: Some("authorization_pending".to_owned()),
            scope: None,
        },
        false,
    )
    .expect("known device-flow error should map to an outcome");

    assert_eq!(pending, TokenPollOutcome::AuthorizationPending);
}

#[test]
fn token_poll_rejects_unsupported_error_codes() {
    let result = token_poll_outcome(
        TokenPollWire { access_token: None, error: Some("invalid_grant".to_owned()), scope: None },
        false,
    );

    assert!(matches!(result, Err(OAuthClientError::UnsupportedOAuthError { .. })));
}

#[test]
fn token_poll_rejects_error_responses_without_error_codes() {
    let result =
        token_poll_outcome(TokenPollWire { access_token: None, error: None, scope: None }, false);

    assert!(matches!(result, Err(OAuthClientError::ReadTokenErrorWithoutErrorField)));
}

#[test]
fn token_poll_rejects_success_without_access_token() {
    let result =
        token_poll_outcome(TokenPollWire { access_token: None, error: None, scope: None }, true);

    assert!(matches!(result, Err(OAuthClientError::MissingAccessToken)));
}

#[test]
fn non_success_device_code_status_is_rejected() {
    let result = super::ensure_device_code_status(StatusCode::BAD_GATEWAY, &tracing::Span::none());

    assert!(matches!(result, Err(OAuthClientError::UnexpectedStatus { .. })));
}
