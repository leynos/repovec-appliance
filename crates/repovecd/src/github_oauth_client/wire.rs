//! Wire models for OAuth device-flow HTTP responses.

use std::time::Duration;

use repovec_core::github_oauth::{
    AccessToken, DeviceAuthorization, DeviceCode, DeviceFlowErrorCode, TokenPollOutcome, UserCode,
};
use serde::Deserialize;

use super::{DEFAULT_DEVICE_POLL_INTERVAL, OAuthClientError};

#[derive(Deserialize)]
pub(super) struct DeviceCodeWire {
    device_code: String,
    user_code: String,
    verification_uri: String,
    expires_in: u64,
    interval: Option<u64>,
}

impl DeviceCodeWire {
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

#[derive(Deserialize)]
pub(super) struct TokenPollWire {
    access_token: Option<String>,
    error: Option<String>,
    scope: Option<String>,
}

impl TokenPollWire {
    pub(super) fn oauth_error(&self) -> Option<TokenErrorWire> {
        self.error.as_ref().map(|error| TokenErrorWire::new(error.clone()))
    }

    pub(super) fn into_domain(self) -> Result<TokenPollOutcome, OAuthClientError> {
        let access_token = self.access_token.ok_or(OAuthClientError::MissingAccessToken)?;
        Ok(TokenPollOutcome::Authorized(AccessToken::new(
            access_token,
            split_scopes(self.scope.as_deref().unwrap_or_default()),
        )))
    }
}

pub(super) struct TokenErrorWire {
    pub(super) raw: String,
    pub(super) code: DeviceFlowErrorCode,
}

impl TokenErrorWire {
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
