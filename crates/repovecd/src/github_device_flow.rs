//! Application orchestration for GitHub OAuth device-flow login.

use std::time::{Duration, Instant};

use repovec_core::github_oauth::{
    AccessToken, ClientId, DeviceAuthorization, PollDecision, TerminalDeviceFlowError,
    TokenPollOutcome, apply_poll_outcome,
};
use thiserror::Error;

/// OAuth API operations required by the device-flow use case.
pub trait DeviceFlowApi {
    /// Error returned by the adapter.
    type Error: std::error::Error + Send + Sync + 'static;

    /// Requests a device authorization from GitHub.
    ///
    /// # Errors
    ///
    /// Returns the adapter error when the request cannot be completed or
    /// interpreted.
    fn request_device_code(
        &self,
        client_id: &ClientId,
        scopes: &[String],
    ) -> Result<DeviceAuthorization, Self::Error>;

    /// Polls the token endpoint once.
    ///
    /// # Errors
    ///
    /// Returns the adapter error when the request cannot be completed or
    /// interpreted.
    fn poll_token(
        &self,
        client_id: &ClientId,
        authorization: &DeviceAuthorization,
    ) -> Result<TokenPollOutcome, Self::Error>;
}

/// Encrypted persistence required by the device-flow use case.
pub trait TokenStore {
    /// Error returned by the storage adapter.
    type Error: std::error::Error + Send + Sync + 'static;

    /// Stores the completed access token.
    ///
    /// # Errors
    ///
    /// Returns the adapter error when the token cannot be encrypted or written.
    fn store(&self, token: &AccessToken) -> Result<(), Self::Error>;
}

/// Sleep boundary used to make polling tests deterministic.
pub trait Sleeper {
    /// Waits for the supplied duration.
    fn sleep(&self, duration: Duration);

    /// Returns elapsed time since this polling run started.
    fn elapsed(&self) -> Duration;
}

/// Production sleeper backed by the current thread.
#[derive(Clone, Debug)]
pub struct ThreadSleeper {
    started_at: Instant,
}

impl ThreadSleeper {
    /// Creates a sleeper whose elapsed time starts now.
    #[must_use]
    pub fn new() -> Self { Self { started_at: Instant::now() } }
}

impl Default for ThreadSleeper {
    fn default() -> Self { Self::new() }
}

impl Sleeper for ThreadSleeper {
    fn sleep(&self, duration: Duration) { std::thread::sleep(duration); }

    fn elapsed(&self) -> Duration { self.started_at.elapsed() }
}

/// User-facing information that should be shown before polling starts.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DeviceLoginPrompt {
    /// URL where the operator enters the user code.
    pub verification_uri: String,
    /// Operator-visible user code.
    pub user_code: String,
}

/// Completed device-flow login result.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CompletedDeviceFlow {
    /// Prompt shown to the operator before polling started.
    pub prompt: DeviceLoginPrompt,
    /// Token returned by GitHub and stored by the token store.
    pub token: AccessToken,
}

/// Input required to start a GitHub OAuth device flow.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DeviceFlowLoginRequest {
    /// GitHub OAuth client identifier.
    pub client_id: ClientId,
    /// Requested OAuth scopes.
    pub scopes: Vec<String>,
}

/// Runs the device flow until it succeeds or reaches a terminal outcome.
///
/// # Errors
///
/// Returns an adapter error for OAuth or storage failures, or a terminal
/// device-flow error when the operator denies access or the code expires.
pub fn complete_device_flow<A, T, S>(
    api: &A,
    store: &T,
    sleeper: &S,
    request: &DeviceFlowLoginRequest,
) -> Result<CompletedDeviceFlow, DeviceFlowRunError<A::Error, T::Error>>
where
    A: DeviceFlowApi,
    T: TokenStore,
    S: Sleeper,
{
    let authorization = api
        .request_device_code(&request.client_id, &request.scopes)
        .map_err(DeviceFlowRunError::OAuth)?;
    let prompt = DeviceLoginPrompt {
        verification_uri: authorization.verification_uri.clone(),
        user_code: authorization.user_code.as_str().to_owned(),
    };
    let mut interval = authorization.interval;

    loop {
        if has_expired(sleeper, authorization.expires_in, interval) {
            return Err(DeviceFlowRunError::Terminal(TerminalDeviceFlowError::ExpiredToken));
        }
        sleeper.sleep(interval);
        let outcome = api
            .poll_token(&request.client_id, &authorization)
            .map_err(DeviceFlowRunError::OAuth)?;
        match apply_poll_outcome(outcome, interval) {
            PollDecision::Continue { next_interval } => {
                interval = next_interval;
            }
            PollDecision::Complete(token) => {
                store.store(&token).map_err(DeviceFlowRunError::Storage)?;
                return Ok(CompletedDeviceFlow { prompt, token });
            }
            PollDecision::Failed(error) => return Err(DeviceFlowRunError::Terminal(error)),
        }
    }
}

fn has_expired<S>(sleeper: &S, expires_in: Duration, next_interval: Duration) -> bool
where
    S: Sleeper,
{
    sleeper.elapsed().saturating_add(next_interval) >= expires_in
}

/// Errors returned while running the device flow.
#[derive(Debug, Error)]
pub enum DeviceFlowRunError<O, S>
where
    O: std::error::Error + Send + Sync + 'static,
    S: std::error::Error + Send + Sync + 'static,
{
    /// The OAuth adapter failed.
    #[error("GitHub OAuth device-flow request failed")]
    OAuth(#[source] O),
    /// The encrypted token store failed.
    #[error("GitHub OAuth token storage failed")]
    Storage(#[source] S),
    /// The OAuth flow reached a terminal response before returning a token.
    #[error("GitHub OAuth device flow ended without a token")]
    Terminal(#[source] TerminalDeviceFlowError),
}

#[cfg(test)]
mod tests {
    //! Tests for device-flow orchestration.

    use std::{cell::RefCell, convert::Infallible, time::Duration};

    use repovec_core::github_oauth::{DeviceCode, TokenPollOutcome, UserCode};
    use rstest::{fixture, rstest};

    use super::{
        AccessToken, ClientId, DeviceAuthorization, DeviceFlowApi, DeviceFlowLoginRequest,
        DeviceFlowRunError, Sleeper, TerminalDeviceFlowError, TokenStore, complete_device_flow,
    };

    struct FakeApi {
        outcomes: RefCell<Vec<TokenPollOutcome>>,
        expires_in: Duration,
    }

    impl DeviceFlowApi for FakeApi {
        type Error = Infallible;

        fn request_device_code(
            &self,
            _client_id: &ClientId,
            _scopes: &[String],
        ) -> Result<DeviceAuthorization, Self::Error> {
            Ok(DeviceAuthorization {
                device_code: DeviceCode::new("device"),
                user_code: UserCode::new("ABCD-1234"),
                verification_uri: "https://github.com/login/device".to_owned(),
                expires_in: self.expires_in,
                interval: Duration::from_secs(5),
            })
        }

        fn poll_token(
            &self,
            _client_id: &ClientId,
            _authorization: &DeviceAuthorization,
        ) -> Result<TokenPollOutcome, Self::Error> {
            Ok(self.outcomes.borrow_mut().remove(0))
        }
    }

    struct FakeStore {
        tokens: RefCell<Vec<AccessToken>>,
    }

    impl TokenStore for FakeStore {
        type Error = Infallible;

        fn store(&self, token: &AccessToken) -> Result<(), Self::Error> {
            self.tokens.borrow_mut().push(token.clone());
            Ok(())
        }
    }

    struct RecordingSleeper {
        sleeps: RefCell<Vec<Duration>>,
        elapsed: RefCell<Duration>,
    }

    impl Sleeper for RecordingSleeper {
        fn sleep(&self, duration: Duration) {
            self.sleeps.borrow_mut().push(duration);
            *self.elapsed.borrow_mut() += duration;
        }

        fn elapsed(&self) -> Duration { *self.elapsed.borrow() }
    }

    #[rstest]
    fn happy_path_stores_the_authorized_token(
        login_request: DeviceFlowLoginRequest,
        recording_sleeper: RecordingSleeper,
    ) {
        let api = FakeApi {
            outcomes: RefCell::new(vec![
                TokenPollOutcome::AuthorizationPending,
                TokenPollOutcome::Authorized(AccessToken::new("gho_secret", ["repo"])),
            ]),
            expires_in: Duration::from_secs(900),
        };
        let store = FakeStore { tokens: RefCell::new(Vec::new()) };

        let result = complete_device_flow(&api, &store, &recording_sleeper, &login_request)
            .expect("device flow should complete");

        assert_eq!(result.prompt.user_code, "ABCD-1234");
        assert_eq!(store.tokens.borrow().len(), 1);
        assert_eq!(
            *recording_sleeper.sleeps.borrow(),
            [Duration::from_secs(5), Duration::from_secs(5)]
        );
    }

    #[rstest]
    fn slow_down_increases_the_next_sleep(
        login_request: DeviceFlowLoginRequest,
        recording_sleeper: RecordingSleeper,
    ) {
        let api = FakeApi {
            outcomes: RefCell::new(vec![
                TokenPollOutcome::SlowDown,
                TokenPollOutcome::Authorized(AccessToken::new("gho_secret", ["repo"])),
            ]),
            expires_in: Duration::from_secs(900),
        };
        let store = FakeStore { tokens: RefCell::new(Vec::new()) };

        complete_device_flow(&api, &store, &recording_sleeper, &login_request)
            .expect("device flow should complete");

        assert_eq!(
            *recording_sleeper.sleeps.borrow(),
            [Duration::from_secs(5), Duration::from_secs(10)]
        );
    }

    #[rstest]
    fn access_denied_stops_without_storing_a_token(
        login_request: DeviceFlowLoginRequest,
        recording_sleeper: RecordingSleeper,
    ) {
        let api = FakeApi {
            outcomes: RefCell::new(vec![TokenPollOutcome::AccessDenied]),
            expires_in: Duration::from_secs(900),
        };
        let store = FakeStore { tokens: RefCell::new(Vec::new()) };

        let result = complete_device_flow(&api, &store, &recording_sleeper, &login_request);

        assert!(matches!(
            result,
            Err(DeviceFlowRunError::Terminal(TerminalDeviceFlowError::AccessDenied)),
        ));
        assert!(store.tokens.borrow().is_empty());
    }

    #[rstest]
    fn local_expiry_stops_polling_before_the_next_sleep_would_exceed_the_deadline(
        login_request: DeviceFlowLoginRequest,
        recording_sleeper: RecordingSleeper,
    ) {
        let api = FakeApi {
            outcomes: RefCell::new(vec![TokenPollOutcome::AuthorizationPending]),
            expires_in: Duration::from_secs(4),
        };
        let store = FakeStore { tokens: RefCell::new(Vec::new()) };

        let result = complete_device_flow(&api, &store, &recording_sleeper, &login_request);

        assert!(matches!(
            result,
            Err(DeviceFlowRunError::Terminal(TerminalDeviceFlowError::ExpiredToken)),
        ));
        assert!(recording_sleeper.sleeps.borrow().is_empty());
    }

    #[fixture]
    fn login_request() -> DeviceFlowLoginRequest {
        DeviceFlowLoginRequest { client_id: ClientId::new("client"), scopes: vec!["repo".into()] }
    }

    #[fixture]
    fn recording_sleeper() -> RecordingSleeper {
        RecordingSleeper { sleeps: RefCell::new(Vec::new()), elapsed: RefCell::new(Duration::ZERO) }
    }
}
