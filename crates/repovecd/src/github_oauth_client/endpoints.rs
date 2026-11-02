//! GitHub OAuth endpoint URLs and their validation.

use oauth2::url::Url;

use super::OAuthClientError;

/// GitHub OAuth endpoint URLs used by the device-flow adapter.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DeviceFlowEndpoints {
    /// URL receiving device-code requests.
    device_code_url: String,
    /// URL receiving token-poll requests.
    token_url: String,
}

impl DeviceFlowEndpoints {
    /// Creates endpoint URLs from explicit strings.
    ///
    /// # Examples
    ///
    /// ```
    /// use repovecd::github_oauth_client::DeviceFlowEndpoints;
    ///
    /// let endpoints = DeviceFlowEndpoints::new(
    ///     "https://github.com/login/device/code",
    ///     "https://github.com/login/oauth/access_token",
    /// );
    ///
    /// assert_eq!(
    ///     endpoints.token_url(),
    ///     "https://github.com/login/oauth/access_token"
    /// );
    /// ```
    #[must_use]
    pub fn new(device_code_url: impl Into<String>, token_url: impl Into<String>) -> Self {
        Self {
            device_code_url: device_code_url.into(),
            token_url: token_url.into(),
        }
    }

    /// Returns the production GitHub device-flow endpoints.
    ///
    /// # Examples
    ///
    /// ```
    /// use repovecd::github_oauth_client::DeviceFlowEndpoints;
    ///
    /// assert_eq!(
    ///     DeviceFlowEndpoints::github().device_code_url(),
    ///     "https://github.com/login/device/code",
    /// );
    /// ```
    #[must_use]
    pub fn github() -> Self {
        Self::new(
            "https://github.com/login/device/code",
            "https://github.com/login/oauth/access_token",
        )
    }

    /// Returns the device-code request URL.
    #[must_use]
    pub fn device_code_url(&self) -> &str { &self.device_code_url }

    /// Returns the token polling URL.
    #[must_use]
    pub fn token_url(&self) -> &str { &self.token_url }
}

/// Checks that the configured token endpoint is a parseable URL.
/// For example, `https://github.com/login/oauth/access_token` is accepted.
pub(super) fn validate_token_url(token_url: &str) -> Result<(), OAuthClientError> {
    Url::parse(token_url)
        .map(|_| ())
        .map_err(OAuthClientError::InvalidTokenUrl)
}

/// Checks that the configured device-code endpoint is a parseable URL.
/// For example, `https://github.com/login/device/code` is accepted.
pub(super) fn validate_device_code_url(device_code_url: &str) -> Result<(), OAuthClientError> {
    Url::parse(device_code_url)
        .map(|_| ())
        .map_err(OAuthClientError::InvalidDeviceCodeUrl)
}
