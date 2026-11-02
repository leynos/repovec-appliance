//! Unit tests for OAuth adapter helpers.

use oauth2::reqwest::StatusCode;
use repovec_core::github_oauth::TokenPollOutcome;

use super::{
    DeviceFlowEndpoints,
    GitHubOAuthClient,
    OAuthClientError,
    TokenPollWire,
    join_scopes,
    token_poll_outcome,
};

#[test]
fn scopes_are_joined_with_spaces() {
    assert_eq!(join_scopes(["repo", "read:org"]), "repo read:org");
}

#[test]
fn invalid_device_code_url_is_rejected() {
    let result = GitHubOAuthClient::new(DeviceFlowEndpoints::new(
        "not a URL",
        "https://example.test/token",
    ));

    assert!(matches!(
        result,
        Err(OAuthClientError::InvalidDeviceCodeUrl(_))
    ));
}

#[test]
fn invalid_token_url_is_rejected() {
    let result = GitHubOAuthClient::new(DeviceFlowEndpoints::new(
        "https://example.test/device",
        "not a URL",
    ));

    assert!(matches!(result, Err(OAuthClientError::InvalidTokenUrl(_))));
}

#[rstest::rstest]
#[case::authorization_pending("authorization_pending", TokenPollOutcome::AuthorizationPending)]
#[case::slow_down("slow_down", TokenPollOutcome::SlowDown)]
#[case::access_denied("access_denied", TokenPollOutcome::AccessDenied)]
#[case::expired_token("expired_token", TokenPollOutcome::ExpiredToken)]
fn token_poll_maps_device_flow_error_codes(
    #[case] error: &str,
    #[case] expected: TokenPollOutcome,
) {
    let outcome = token_poll_outcome(
        TokenPollWire {
            access_token: None,
            error: Some(error.to_owned()),
            scope: None,
        },
        false,
    )
    .expect("known device-flow error should map to an outcome");

    assert_eq!(outcome, expected);
}

#[derive(Clone, Copy, Debug)]
enum ExpectedPollError {
    UnsupportedError,
    MissingErrorField,
    MissingAccessToken,
}

impl ExpectedPollError {
    fn assert_matches(self, error: &OAuthClientError) {
        let matches_expected = match self {
            Self::UnsupportedError => {
                matches!(error, OAuthClientError::UnsupportedOAuthError { .. })
            }
            Self::MissingErrorField => {
                matches!(error, OAuthClientError::ReadTokenErrorWithoutErrorField)
            }
            Self::MissingAccessToken => matches!(error, OAuthClientError::MissingAccessToken),
        };

        assert!(matches_expected, "expected {self:?}, got {error:?}");
    }
}

#[rstest::rstest]
#[case::unsupported_error_code(
    TokenPollWire {
        access_token: None,
        error: Some("invalid_grant".to_owned()),
        scope: None,
    },
    false,
    ExpectedPollError::UnsupportedError,
)]
#[case::missing_error_code(
    TokenPollWire {
        access_token: None,
        error: None,
        scope: None,
    },
    false,
    ExpectedPollError::MissingErrorField,
)]
#[case::missing_access_token(
    TokenPollWire {
        access_token: None,
        error: None,
        scope: None,
    },
    true,
    ExpectedPollError::MissingAccessToken,
)]
fn token_poll_rejects_invalid_responses(
    #[case] body: TokenPollWire,
    #[case] is_success: bool,
    #[case] expected_error: ExpectedPollError,
) {
    let result = token_poll_outcome(body, is_success);

    let Err(error) = result else {
        panic!("invalid token response should be rejected");
    };
    expected_error.assert_matches(&error);
}

#[test]
fn non_success_device_code_status_is_rejected() {
    let result = super::ensure_device_code_status(StatusCode::BAD_GATEWAY, &tracing::Span::none());

    assert!(matches!(
        result,
        Err(OAuthClientError::UnexpectedStatus { .. })
    ));
}
