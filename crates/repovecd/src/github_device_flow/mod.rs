//! Application orchestration for GitHub OAuth device-flow login.

use std::time::{Duration, Instant};

use repovec_core::github_oauth::{
    AccessToken, ClientId, DeviceAuthorization, PollDecision, TerminalDeviceFlowError,
    TokenPollOutcome, UserCode, apply_poll_outcome,
};
use thiserror::Error;
use tracing::info;

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
    pub user_code: UserCode,
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

/// Runtime adapters required by one device-flow login attempt.
#[derive(Clone, Copy, Debug)]
pub struct DeviceFlowRuntime<'a, A, T, S>
where
    A: DeviceFlowApi,
    T: TokenStore,
    S: Sleeper,
{
    /// OAuth protocol adapter.
    pub api: &'a A,
    /// Encrypted token persistence adapter.
    pub store: &'a T,
    /// Sleep and elapsed-time adapter.
    pub sleeper: &'a S,
}

impl<'a, A, T, S> DeviceFlowRuntime<'a, A, T, S>
where
    A: DeviceFlowApi,
    T: TokenStore,
    S: Sleeper,
{
    /// Creates a runtime adapter bundle for a device-flow login attempt.
    #[must_use]
    pub const fn new(api: &'a A, store: &'a T, sleeper: &'a S) -> Self {
        Self { api, store, sleeper }
    }
}

/// Runs the device flow until it succeeds or reaches a terminal outcome.
///
/// # Errors
///
/// Returns an adapter error for OAuth or storage failures, or a terminal
/// device-flow error when the operator denies access or the code expires.
pub fn complete_device_flow<A, T, S, P>(
    runtime: &DeviceFlowRuntime<'_, A, T, S>,
    request: &DeviceFlowLoginRequest,
    present_prompt: P,
) -> Result<CompletedDeviceFlow, DeviceFlowRunError<A::Error, T::Error>>
where
    A: DeviceFlowApi,
    T: TokenStore,
    S: Sleeper,
    P: FnOnce(&DeviceLoginPrompt),
{
    let (authorization, prompt) = begin_device_flow(runtime, request, present_prompt)?;
    let token = poll_until_token(runtime, request, &authorization)?;
    store_completed_token(runtime, &token)?;
    Ok(CompletedDeviceFlow { prompt, token })
}

fn begin_device_flow<A, T, S, P>(
    runtime: &DeviceFlowRuntime<'_, A, T, S>,
    request: &DeviceFlowLoginRequest,
    present_prompt: P,
) -> Result<(DeviceAuthorization, DeviceLoginPrompt), DeviceFlowRunError<A::Error, T::Error>>
where
    A: DeviceFlowApi,
    T: TokenStore,
    S: Sleeper,
    P: FnOnce(&DeviceLoginPrompt),
{
    info!(scope_count = request.scopes.len(), "requesting GitHub device code");
    let authorization = runtime
        .api
        .request_device_code(&request.client_id, &request.scopes)
        .map_err(DeviceFlowRunError::OAuth)?;
    let prompt = DeviceLoginPrompt {
        verification_uri: authorization.verification_uri.clone(),
        user_code: authorization.user_code.clone(),
    };
    present_prompt(&prompt);
    Ok((authorization, prompt))
}

fn store_completed_token<A, T, S>(
    runtime: &DeviceFlowRuntime<'_, A, T, S>,
    token: &AccessToken,
) -> Result<(), DeviceFlowRunError<A::Error, T::Error>>
where
    A: DeviceFlowApi,
    T: TokenStore,
    S: Sleeper,
{
    runtime.store.store(token).map_err(DeviceFlowRunError::Storage)?;
    info!("stored GitHub OAuth access token");
    Ok(())
}

fn poll_until_token<A, T, S>(
    runtime: &DeviceFlowRuntime<'_, A, T, S>,
    request: &DeviceFlowLoginRequest,
    authorization: &DeviceAuthorization,
) -> Result<AccessToken, DeviceFlowRunError<A::Error, T::Error>>
where
    A: DeviceFlowApi,
    T: TokenStore,
    S: Sleeper,
{
    let mut polling = ActivePolling::new(authorization.interval);
    info_polling_started(&polling, authorization.expires_in);

    loop {
        ensure_polling_not_expired(runtime.sleeper, authorization.expires_in, polling.interval)
            .map_err(DeviceFlowRunError::Terminal)?;
        let decision = poll_once(runtime, request, authorization, &mut polling)?;
        if let Some(token) = apply_poll_decision(decision, &mut polling)? {
            return Ok(token);
        }
    }
}

fn ensure_polling_not_expired<S>(
    sleeper: &S,
    expires_in: Duration,
    next_interval: Duration,
) -> Result<(), TerminalDeviceFlowError>
where
    S: Sleeper,
{
    if has_expired(sleeper, expires_in, next_interval) {
        info_polling_expired(sleeper, next_interval, expires_in);
        return Err(TerminalDeviceFlowError::ExpiredToken);
    }
    Ok(())
}

fn poll_once<A, T, S>(
    runtime: &DeviceFlowRuntime<'_, A, T, S>,
    request: &DeviceFlowLoginRequest,
    authorization: &DeviceAuthorization,
    polling: &mut ActivePolling,
) -> Result<PollDecision, DeviceFlowRunError<A::Error, T::Error>>
where
    A: DeviceFlowApi,
    T: TokenStore,
    S: Sleeper,
{
    runtime.sleeper.sleep(polling.interval);
    polling.record_attempt();
    runtime
        .api
        .poll_token(&request.client_id, authorization)
        .map(|outcome| apply_poll_outcome(outcome, polling.interval))
        .map_err(DeviceFlowRunError::OAuth)
}

fn apply_poll_decision<O, S>(
    decision: PollDecision,
    polling: &mut ActivePolling,
) -> Result<Option<AccessToken>, DeviceFlowRunError<O, S>>
where
    O: std::error::Error + Send + Sync + 'static,
    S: std::error::Error + Send + Sync + 'static,
{
    match decision {
        PollDecision::Continue { next_interval } => {
            polling.update_interval(next_interval);
            Ok(None)
        }
        PollDecision::Complete(token) => Ok(Some(token)),
        PollDecision::Failed(error) => {
            info!(attempt = polling.attempt, ?error, "GitHub device-flow terminal outcome");
            Err(DeviceFlowRunError::Terminal(error))
        }
    }
}

#[derive(Clone, Copy, Debug)]
struct ActivePolling {
    interval: Duration,
    attempt: u64,
}

impl ActivePolling {
    const fn new(interval: Duration) -> Self { Self { interval, attempt: 0 } }

    const fn record_attempt(&mut self) { self.attempt = self.attempt.saturating_add(1); }

    fn update_interval(&mut self, next_interval: Duration) {
        if next_interval > self.interval {
            info!(
                attempt = self.attempt,
                previous_interval_seconds = self.interval.as_secs(),
                next_interval_seconds = next_interval.as_secs(),
                "GitHub device-flow slow_down adjusted the polling interval",
            );
        }
        self.interval = next_interval;
    }
}

fn info_polling_started(polling: &ActivePolling, expires_in: Duration) {
    info!(
        interval_seconds = polling.interval.as_secs(),
        expires_in_seconds = expires_in.as_secs(),
        "entering GitHub device-flow polling loop",
    );
}

fn info_polling_expired<S>(sleeper: &S, next_interval: Duration, expires_in: Duration)
where
    S: Sleeper,
{
    info!(
        elapsed_seconds = sleeper.elapsed().as_secs(),
        next_interval_seconds = next_interval.as_secs(),
        expires_in_seconds = expires_in.as_secs(),
        "GitHub device-flow authorization expired before next poll",
    );
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
mod tests;
