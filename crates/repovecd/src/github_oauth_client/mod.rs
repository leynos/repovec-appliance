//! HTTP adapter for GitHub OAuth device-flow endpoints.

use std::time::{Duration, Instant};

use oauth2::{
    reqwest::{
        StatusCode,
        blocking::{Client, Response},
        redirect::Policy,
    },
    url::Url,
};
use repovec_core::github_oauth::{
    ClientId, DeviceAuthorization, DeviceCode, TokenPollOutcome, classify_device_flow_error,
};
use thiserror::Error;
use tracing::{Span, info_span};

use crate::github_device_flow::DeviceFlowApi;

mod observability;
mod wire;

use observability::{
    info_adapter_failure, info_http_request, info_token_poll, info_token_poll_outcome,
};
use wire::{DeviceCodeWire, TokenPollWire};

const DEVICE_CODE_GRANT_TYPE: &str = "urn:ietf:params:oauth:grant-type:device_code";
const DEFAULT_DEVICE_POLL_INTERVAL: Duration = Duration::from_secs(5);
const HTTP_CONNECT_TIMEOUT: Duration = Duration::from_secs(10);
const HTTP_REQUEST_TIMEOUT: Duration = Duration::from_secs(30);

/// GitHub OAuth endpoint URLs used by the device-flow adapter.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DeviceFlowEndpoints {
    device_code_url: String,
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
    /// assert_eq!(endpoints.token_url(), "https://github.com/login/oauth/access_token");
    /// ```
    #[must_use]
    pub fn new(device_code_url: impl Into<String>, token_url: impl Into<String>) -> Self {
        Self { device_code_url: device_code_url.into(), token_url: token_url.into() }
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

/// Blocking OAuth device-flow client used by repovecd.
#[derive(Debug)]
pub struct GitHubOAuthClient {
    endpoints: DeviceFlowEndpoints,
    http: Client,
}

impl GitHubOAuthClient {
    /// Creates a client for the supplied endpoints.
    ///
    /// # Errors
    ///
    /// Returns an error if either endpoint URL is invalid or the underlying
    /// HTTP client cannot be constructed.
    ///
    /// # Examples
    ///
    /// ```
    /// use repovecd::github_oauth_client::{DeviceFlowEndpoints, GitHubOAuthClient};
    /// GitHubOAuthClient::new(DeviceFlowEndpoints::github())?;
    ///
    /// # Ok::<(), repovecd::github_oauth_client::OAuthClientError>(())
    /// ```
    pub fn new(endpoints: DeviceFlowEndpoints) -> Result<Self, OAuthClientError> {
        validate_device_code_url(endpoints.device_code_url())?;
        validate_token_url(endpoints.token_url())?;
        let http = Client::builder()
            .redirect(Policy::none())
            .connect_timeout(HTTP_CONNECT_TIMEOUT)
            .timeout(HTTP_REQUEST_TIMEOUT)
            .build()
            .map_err(OAuthClientError::BuildHttpClient)?;
        Ok(Self { endpoints, http })
    }

    /// Requests a device code and user code from the authorization server.
    ///
    /// # Errors
    ///
    /// Returns a typed error for transport failures, non-success server
    /// responses, or malformed response bodies.
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// use repovec_core::github_oauth::ClientId;
    /// use repovecd::github_oauth_client::{DeviceFlowEndpoints, GitHubOAuthClient};
    ///
    /// let client = GitHubOAuthClient::new(DeviceFlowEndpoints::github())?;
    /// let authorization =
    ///     client.request_device_code(&ClientId::new("github-client-id"), ["repo"])?;
    /// assert_eq!(authorization.interval.as_secs(), 5);
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn request_device_code<I, S>(
        &self,
        client_id: &ClientId,
        scopes: I,
    ) -> Result<DeviceAuthorization, OAuthClientError>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        let scope = join_scopes(scopes);
        let span =
            info_span!("github_oauth.request_device_code", scope_count = scope_count(&scope));
        let started_at = Instant::now();
        let response = self.send_device_code_request(&span, client_id, &scope)?;
        info_http_request(&span, response.status(), started_at);
        ensure_device_code_status(response.status(), &span)?;

        let wire = response.json::<DeviceCodeWire>().map_err(OAuthClientError::ReadDeviceCode)?;
        Ok(wire.into_domain())
    }

    /// Polls the token endpoint exactly once.
    ///
    /// # Errors
    ///
    /// Returns a typed error for transport failures, malformed success or
    /// error responses, and OAuth errors that are not part of the device-flow
    /// polling contract.
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// use repovec_core::github_oauth::{ClientId, DeviceCode, TokenPollOutcome};
    /// use repovecd::github_oauth_client::{DeviceFlowEndpoints, GitHubOAuthClient};
    ///
    /// let client = GitHubOAuthClient::new(DeviceFlowEndpoints::github())?;
    /// let outcome =
    ///     client.poll_token(&ClientId::new("github-client-id"), &DeviceCode::new("device-code"))?;
    /// match outcome {
    ///     TokenPollOutcome::Authorized(token) => {
    ///         assert!(token.scopes().iter().any(|scope| scope == "repo"));
    ///     }
    ///     TokenPollOutcome::AuthorizationPending => {}
    ///     TokenPollOutcome::SlowDown => {}
    ///     TokenPollOutcome::AccessDenied => {}
    ///     TokenPollOutcome::ExpiredToken => {}
    /// }
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn poll_token(
        &self,
        client_id: &ClientId,
        device_code: &DeviceCode,
    ) -> Result<TokenPollOutcome, OAuthClientError> {
        let span = info_span!("github_oauth.poll_token");
        let started_at = Instant::now();
        let response = self.send_token_poll_request(&span, client_id, device_code)?;
        info_token_poll(&span, response.status(), started_at);

        let is_success = response.status().is_success();
        let body = response.json::<TokenPollWire>().map_err(OAuthClientError::ReadToken)?;
        let outcome = token_poll_outcome(body, is_success)?;
        info_token_poll_outcome(&span, &outcome);
        Ok(outcome)
    }

    fn send_device_code_request(
        &self,
        span: &Span,
        client_id: &ClientId,
        scope: &str,
    ) -> Result<Response, OAuthClientError> {
        span.in_scope(|| {
            self.http
                .post(self.endpoints.device_code_url())
                .header("Accept", "application/json")
                .form(&[("client_id", client_id.as_str()), ("scope", scope)])
                .send()
        })
        .map_err(OAuthClientError::RequestDeviceCode)
    }

    fn send_token_poll_request(
        &self,
        span: &Span,
        client_id: &ClientId,
        device_code: &DeviceCode,
    ) -> Result<Response, OAuthClientError> {
        span.in_scope(|| {
            self.http
                .post(self.endpoints.token_url())
                .header("Accept", "application/json")
                .form(&[
                    ("client_id", client_id.as_str()),
                    ("device_code", device_code.secret()),
                    ("grant_type", DEVICE_CODE_GRANT_TYPE),
                ])
                .send()
        })
        .map_err(OAuthClientError::PollToken)
    }
}

impl DeviceFlowApi for GitHubOAuthClient {
    type Error = OAuthClientError;

    fn request_device_code(
        &self,
        client_id: &ClientId,
        scopes: &[String],
    ) -> Result<DeviceAuthorization, Self::Error> {
        self.request_device_code(client_id, scopes)
    }

    fn poll_token(
        &self,
        client_id: &ClientId,
        authorization: &DeviceAuthorization,
    ) -> Result<TokenPollOutcome, Self::Error> {
        self.poll_token(client_id, &authorization.device_code)
    }
}

/// Errors returned by the OAuth HTTP adapter.
#[derive(Debug, Error)]
pub enum OAuthClientError {
    /// The HTTP client could not be built.
    #[error("failed to build OAuth HTTP client")]
    BuildHttpClient(#[source] oauth2::reqwest::Error),
    /// The device-code URL is invalid.
    #[error("invalid OAuth device-code URL")]
    InvalidDeviceCodeUrl(#[source] oauth2::url::ParseError),
    /// The token URL is invalid.
    #[error("invalid OAuth token URL")]
    InvalidTokenUrl(#[source] oauth2::url::ParseError),
    /// The device-code request failed before a response was received.
    #[error("failed to request GitHub device code")]
    RequestDeviceCode(#[source] oauth2::reqwest::Error),
    /// The token poll failed before a response was received.
    #[error("failed to poll GitHub device-flow token endpoint")]
    PollToken(#[source] oauth2::reqwest::Error),
    /// The server returned an unexpected non-success status.
    #[error("GitHub OAuth {endpoint} endpoint returned unexpected status {status}")]
    UnexpectedStatus {
        /// Logical endpoint name.
        endpoint: &'static str,
        /// HTTP status code returned by the server.
        status: StatusCode,
    },
    /// The device-code response body was malformed.
    #[error("GitHub OAuth device-code response was malformed")]
    ReadDeviceCode(#[source] oauth2::reqwest::Error),
    /// The successful token response body was malformed.
    #[error("GitHub OAuth token response was malformed")]
    ReadToken(#[source] oauth2::reqwest::Error),
    /// The token error response body did not identify an OAuth error.
    #[error("GitHub OAuth token error response did not include an error code")]
    ReadTokenErrorWithoutErrorField,
    /// The token response body did not include an access token.
    #[error("GitHub OAuth token response did not include an access token")]
    MissingAccessToken,
    /// The OAuth server returned an error outside the device-flow contract.
    #[error("GitHub OAuth returned unsupported error `{error}`")]
    UnsupportedOAuthError {
        /// OAuth error code.
        error: String,
    },
}

fn ensure_device_code_status(status: StatusCode, span: &Span) -> Result<(), OAuthClientError> {
    if status.is_success() {
        return Ok(());
    }
    info_adapter_failure(span, status);
    Err(OAuthClientError::UnexpectedStatus { endpoint: "device-code", status })
}

fn token_poll_outcome(
    body: TokenPollWire,
    is_success: bool,
) -> Result<TokenPollOutcome, OAuthClientError> {
    if let Some(error) = body.oauth_error() {
        return classify_device_flow_error(error.code)
            .ok_or(OAuthClientError::UnsupportedOAuthError { error: error.raw });
    }
    if is_success {
        return body.into_domain();
    }
    Err(OAuthClientError::ReadTokenErrorWithoutErrorField)
}

fn scope_count(scopes: &str) -> usize {
    scopes.split_whitespace().filter(|scope| !scope.is_empty()).count()
}

fn validate_token_url(token_url: &str) -> Result<(), OAuthClientError> {
    Url::parse(token_url).map(|_| ()).map_err(OAuthClientError::InvalidTokenUrl)
}

fn validate_device_code_url(device_code_url: &str) -> Result<(), OAuthClientError> {
    Url::parse(device_code_url).map(|_| ()).map_err(OAuthClientError::InvalidDeviceCodeUrl)
}

fn join_scopes<I, S>(scopes: I) -> String
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    let mut joined = String::new();
    for scope in scopes {
        if !joined.is_empty() {
            joined.push(' ');
        }
        joined.push_str(scope.as_ref());
    }
    joined
}

#[cfg(test)]
mod tests {
    //! Unit tests for OAuth adapter helpers.

    use super::join_scopes;

    #[test]
    fn scopes_are_joined_with_spaces() {
        assert_eq!(join_scopes(["repo", "read:org"]), "repo read:org");
    }
}
