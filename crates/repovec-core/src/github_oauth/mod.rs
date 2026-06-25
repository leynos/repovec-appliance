//! GitHub OAuth device-flow policy and domain types.

use std::{fmt, time::Duration};

/// Minimum extra delay required by RFC 8628 after a `slow_down` response.
pub const SLOW_DOWN_EXTRA_DELAY: Duration = Duration::from_secs(5);

/// OAuth client identifier issued for the repovec GitHub OAuth app.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ClientId(String);

impl ClientId {
    /// Creates a client identifier.
    ///
    /// # Examples
    ///
    /// ```
    /// use repovec_core::github_oauth::ClientId;
    ///
    /// assert_eq!(ClientId::new("client").as_str(), "client");
    /// ```
    #[must_use]
    pub fn new(value: impl Into<String>) -> Self { Self(value.into()) }

    /// Returns the raw client identifier.
    ///
    /// # Examples
    ///
    /// ```
    /// use repovec_core::github_oauth::ClientId;
    ///
    /// assert_eq!(ClientId::new("client").as_str(), "client");
    /// ```
    #[must_use]
    pub fn as_str(&self) -> &str { &self.0 }
}

/// Device authorization details returned before an operator approves access.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DeviceAuthorization {
    /// Secret code sent only to the OAuth token endpoint.
    pub device_code: DeviceCode,
    /// Short code shown to the operator.
    pub user_code: UserCode,
    /// URL where the operator enters the user code.
    pub verification_uri: String,
    /// Remaining lifetime of the device and user codes.
    pub expires_in: Duration,
    /// Minimum delay between token polls.
    pub interval: Duration,
}

/// Secret device code used when polling for the token.
#[derive(Clone, Eq, PartialEq)]
pub struct DeviceCode(String);

impl DeviceCode {
    /// Creates a device code.
    ///
    /// # Examples
    ///
    /// ```
    /// use repovec_core::github_oauth::DeviceCode;
    ///
    /// assert_eq!(DeviceCode::new("device").secret(), "device");
    /// ```
    #[must_use]
    pub fn new(value: impl Into<String>) -> Self { Self(value.into()) }

    /// Returns the secret value for adapter use.
    ///
    /// # Examples
    ///
    /// ```
    /// use repovec_core::github_oauth::DeviceCode;
    ///
    /// assert_eq!(DeviceCode::new("device").secret(), "device");
    /// ```
    #[must_use]
    pub fn secret(&self) -> &str { &self.0 }
}

impl fmt::Debug for DeviceCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { f.write_str("DeviceCode(REDACTED)") }
}

/// User-facing code entered at GitHub's device login page.
#[derive(Clone, Eq, PartialEq)]
pub struct UserCode(String);

impl UserCode {
    /// Creates a user code.
    ///
    /// # Examples
    ///
    /// ```
    /// use repovec_core::github_oauth::UserCode;
    ///
    /// assert_eq!(UserCode::new("ABCD-1234").as_str(), "ABCD-1234");
    /// ```
    #[must_use]
    pub fn new(value: impl Into<String>) -> Self { Self(value.into()) }

    /// Returns the operator-visible user code.
    ///
    /// # Examples
    ///
    /// ```
    /// use repovec_core::github_oauth::UserCode;
    ///
    /// assert_eq!(UserCode::new("ABCD-1234").as_str(), "ABCD-1234");
    /// ```
    #[must_use]
    pub fn as_str(&self) -> &str { &self.0 }
}

impl fmt::Debug for UserCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { f.write_str("UserCode(REDACTED)") }
}

/// GitHub access token returned by a completed device flow.
#[derive(Clone, Eq, PartialEq)]
pub struct AccessToken {
    token: String,
    scopes: Vec<String>,
}

impl AccessToken {
    /// Creates an access token with the scopes granted by the server.
    ///
    /// # Examples
    ///
    /// ```
    /// use repovec_core::github_oauth::AccessToken;
    ///
    /// let token = AccessToken::new("gho_token", ["repo"]);
    ///
    /// assert_eq!(token.secret(), "gho_token");
    /// ```
    #[must_use]
    pub fn new<I, S>(token: impl Into<String>, scopes: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        Self { token: token.into(), scopes: scopes.into_iter().map(Into::into).collect() }
    }

    /// Returns the secret bearer token for storage or API use.
    ///
    /// # Examples
    ///
    /// ```
    /// use repovec_core::github_oauth::AccessToken;
    ///
    /// assert_eq!(AccessToken::new("gho_token", ["repo"]).secret(), "gho_token");
    /// ```
    #[must_use]
    pub fn secret(&self) -> &str { &self.token }

    /// Returns the granted scopes.
    ///
    /// # Examples
    ///
    /// ```
    /// use repovec_core::github_oauth::AccessToken;
    ///
    /// let token = AccessToken::new("gho_token", ["repo"]);
    ///
    /// assert_eq!(token.scopes(), ["repo"]);
    /// ```
    #[must_use]
    pub fn scopes(&self) -> &[String] { &self.scopes }
}

impl fmt::Debug for AccessToken {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AccessToken")
            .field("token", &"REDACTED")
            .field("scopes", &self.scopes)
            .finish()
    }
}

/// Outcome returned after polling the token endpoint once.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TokenPollOutcome {
    /// The operator approved the flow and GitHub returned an access token.
    Authorized(AccessToken),
    /// The operator has not approved the flow yet.
    AuthorizationPending,
    /// The server asked the client to poll less frequently.
    SlowDown,
    /// The operator denied the authorization request.
    AccessDenied,
    /// The device code expired before approval.
    ExpiredToken,
}

/// Device-flow error code returned by an OAuth token endpoint.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DeviceFlowErrorCode {
    /// The operator has not approved the flow yet.
    AuthorizationPending,
    /// The server asked the client to poll less frequently.
    SlowDown,
    /// The operator denied the authorization request.
    AccessDenied,
    /// The device code expired before approval.
    ExpiredToken,
    /// The OAuth server returned an error outside the device-flow contract.
    Unsupported,
}

/// Next action after interpreting a token-poll response.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PollDecision {
    /// Continue polling after the supplied interval.
    Continue {
        /// Minimum delay before the next token poll.
        next_interval: Duration,
    },
    /// Finish the flow successfully.
    Complete(AccessToken),
    /// Finish the flow without a token.
    Failed(TerminalDeviceFlowError),
}

/// Terminal failure returned by the device flow.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TerminalDeviceFlowError {
    /// The operator denied access in the browser.
    AccessDenied,
    /// The device code expired and a new flow must be started.
    ExpiredToken,
}

impl fmt::Display for TerminalDeviceFlowError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::AccessDenied => f.write_str("access was denied"),
            Self::ExpiredToken => f.write_str("the device code expired"),
        }
    }
}

impl std::error::Error for TerminalDeviceFlowError {}

/// Applies one polling outcome to the active polling interval.
///
/// # Examples
///
/// ```
/// use std::time::Duration;
/// use repovec_core::github_oauth::{PollDecision, TokenPollOutcome, apply_poll_outcome};
///
/// let decision = apply_poll_outcome(TokenPollOutcome::SlowDown, Duration::from_secs(5));
///
/// assert_eq!(
///     decision,
///     PollDecision::Continue { next_interval: Duration::from_secs(10) },
/// );
/// ```
#[must_use]
pub fn apply_poll_outcome(outcome: TokenPollOutcome, active_interval: Duration) -> PollDecision {
    match outcome {
        TokenPollOutcome::Authorized(token) => PollDecision::Complete(token),
        TokenPollOutcome::AuthorizationPending => {
            PollDecision::Continue { next_interval: active_interval }
        }
        TokenPollOutcome::SlowDown => PollDecision::Continue {
            next_interval: active_interval.saturating_add(SLOW_DOWN_EXTRA_DELAY),
        },
        TokenPollOutcome::AccessDenied => {
            PollDecision::Failed(TerminalDeviceFlowError::AccessDenied)
        }
        TokenPollOutcome::ExpiredToken => {
            PollDecision::Failed(TerminalDeviceFlowError::ExpiredToken)
        }
    }
}

/// Converts a device-flow error code into the domain polling outcome.
///
/// # Examples
///
/// ```
/// use repovec_core::github_oauth::{
///     DeviceFlowErrorCode, TokenPollOutcome, classify_device_flow_error,
/// };
///
/// assert_eq!(
///     classify_device_flow_error(DeviceFlowErrorCode::AuthorizationPending),
///     Some(TokenPollOutcome::AuthorizationPending),
/// );
/// ```
#[must_use]
pub const fn classify_device_flow_error(error: DeviceFlowErrorCode) -> Option<TokenPollOutcome> {
    match error {
        DeviceFlowErrorCode::AuthorizationPending => Some(TokenPollOutcome::AuthorizationPending),
        DeviceFlowErrorCode::SlowDown => Some(TokenPollOutcome::SlowDown),
        DeviceFlowErrorCode::AccessDenied => Some(TokenPollOutcome::AccessDenied),
        DeviceFlowErrorCode::ExpiredToken => Some(TokenPollOutcome::ExpiredToken),
        DeviceFlowErrorCode::Unsupported => None,
    }
}

#[cfg(test)]
mod tests;
