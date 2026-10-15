//! Wire models for OAuth device-flow HTTP responses.

use std::time::Duration;

use repovec_core::github_oauth::{
    AccessToken, DeviceAuthorization, DeviceCode, DeviceFlowErrorCode, TokenPollOutcome, UserCode,
};
use serde::Deserialize;

use super::{DEFAULT_DEVICE_POLL_INTERVAL, OAuthClientError};

/// Deserializes the server response that starts a device authorization.
#[derive(Deserialize)]
pub(super) struct DeviceCodeWire {
    /// Server-issued device code, converted into the secret-bearing domain type.
    device_code: String,
    /// Short code the user enters at the verification page.
    user_code: String,
    /// Page where the user approves the device authorization.
    verification_uri: String,
    /// Authorization lifetime in seconds from the response.
    expires_in: u64,
    /// Optional polling interval in seconds; absence selects the local default.
    interval: Option<u64>,
}

impl DeviceCodeWire {
    /// Converts response seconds and strings into the domain authorization.
    /// For example, an omitted interval uses the five-second default.
    pub(super) fn into_domain(self) -> DeviceAuthorization {
        DeviceAuthorization {
            device_code: DeviceCode::new(self.device_code),
            user_code: UserCode::new(self.user_code),
            verification_uri: self.verification_uri,
            expires_in: Duration::from_secs(self.expires_in),
            interval: self.interval.map_or(DEFAULT_DEVICE_POLL_INTERVAL, Duration::from_secs),
        }
    }
}

/// Deserializes the optional fields returned by the token endpoint.
#[derive(Deserialize)]
pub(super) struct TokenPollWire {
    /// Token returned on authorization; conversion moves it into the domain type.
    pub(super) access_token: Option<String>,
    /// OAuth error code returned for pending, denied, or expired polls.
    pub(super) error: Option<String>,
    /// Granted scopes, accepted as either comma- or space-separated text.
    pub(super) scope: Option<String>,
}

impl TokenPollWire {
    /// Parses the optional OAuth error into its raw and classified forms.
    /// For example, `slow_down` is classified as the device-flow slowdown.
    pub(super) fn oauth_error(&self) -> Option<TokenErrorWire> {
        self.error.as_ref().map(|error| TokenErrorWire::new(error.clone()))
    }

    /// Converts a successful response, requiring an access token.
    /// For example, missing `access_token` returns `MissingAccessToken`.
    pub(super) fn into_domain(self) -> Result<TokenPollOutcome, OAuthClientError> {
        let access_token = self.access_token.ok_or(OAuthClientError::MissingAccessToken)?;
        Ok(TokenPollOutcome::Authorized(AccessToken::new(
            access_token,
            split_scopes(self.scope.as_deref().unwrap_or_default()),
        )))
    }
}

/// Keeps the server error text alongside its device-flow classification.
pub(super) struct TokenErrorWire {
    /// Original OAuth error text used when reporting unsupported codes.
    pub(super) raw: String,
    /// Device-flow category used to select a polling outcome.
    pub(super) code: DeviceFlowErrorCode,
}

impl TokenErrorWire {
    /// Classifies a raw OAuth error while preserving the original text.
    /// For example, `authorization_pending` maps to its pending category.
    fn new(raw: String) -> Self {
        let code = match raw.as_str() {
            "authorization_pending" => DeviceFlowErrorCode::AuthorizationPending,
            "slow_down" => DeviceFlowErrorCode::SlowDown,
            "access_denied" => DeviceFlowErrorCode::AccessDenied,
            "expired_token" => DeviceFlowErrorCode::ExpiredToken,
            _ => DeviceFlowErrorCode::Unsupported,
        };
        Self { raw, code }
    }
}

/// Splits returned scopes on commas or spaces and drops empty entries.
/// For example, `repo,read:org gist` yields three scope names.
fn split_scopes(scopes: &str) -> impl Iterator<Item = String> + '_ {
    scopes.split([',', ' ']).filter(|scope| !scope.is_empty()).map(str::to_owned)
}

#[cfg(test)]
mod tests {
    //! Unit tests for OAuth wire parsing helpers.

    use super::split_scopes;

    #[test]
    fn returned_scopes_accept_github_commas_and_oauth_spaces() {
        let scopes = split_scopes("repo,read:org gist").collect::<Vec<_>>();

        assert_eq!(scopes, ["repo", "read:org", "gist"]);
    }
}
