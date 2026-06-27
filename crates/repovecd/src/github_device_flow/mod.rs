//! Application orchestration for GitHub OAuth device-flow login.

use std::time::{Duration, Instant};

use repovec_core::github_oauth::{
    AccessToken, ClientId, DeviceAuthorization, PollDecision, TerminalDeviceFlowError,
    TokenPollOutcome, UserCode, apply_poll_outcome,
};
use thiserror::Error;
use tracing::info;

mod clock;
mod observability;

pub use clock::{MonotonicClock, StdMonotonicClock};
use observability::{
    device_flow_span, info_device_flow_result, info_device_flow_started, info_interval_increase,
    info_terminal_outcome,
};

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
}

static STD_MONOTONIC_CLOCK: StdMonotonicClock = StdMonotonicClock;

/// Production sleeper backed by the current thread.
#[derive(Clone, Debug)]
pub struct ThreadSleeper;

impl ThreadSleeper {
    /// Creates a sleeper backed by the current thread.
    #[must_use]
    pub const fn new() -> Self { Self }
}

impl Default for ThreadSleeper {
    fn default() -> Self { Self::new() }
}

impl Sleeper for ThreadSleeper {
    fn sleep(&self, duration: Duration) { std::thread::sleep(duration); }
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

/// Runtime adapters for one device-flow login attempt.
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
    /// Delay adapter used between token polling attempts.
    pub sleeper: &'a S,
    /// Monotonic time adapter used for device authorization expiry.
    pub clock: &'a dyn MonotonicClock,
}

impl<'a, A, T, S> DeviceFlowRuntime<'a, A, T, S>
where
    A: DeviceFlowApi,
    T: TokenStore,
    S: Sleeper,
{
    /// Creates a runtime adapter bundle.
    #[must_use]
    pub const fn new(api: &'a A, store: &'a T, sleeper: &'a S) -> Self {
        Self { api, store, sleeper, clock: &STD_MONOTONIC_CLOCK }
    }

    /// Creates a runtime adapter bundle with an explicit clock.
    #[must_use]
    pub const fn with_clock(
        api: &'a A,
        store: &'a T,
        sleeper: &'a S,
        clock: &'a dyn MonotonicClock,
    ) -> Self {
        Self { api, store, sleeper, clock }
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
    let flow_span = device_flow_span(request);
    let _entered = flow_span.enter();
    info_device_flow_started();
    let result = complete_device_flow_in_span(runtime, request, present_prompt);
    info_device_flow_result(&result);
    result
}

fn complete_device_flow_in_span<A, T, S, P>(
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
    info!("metric.github_device_flow_storage_success_total");
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
    let mut polling = ActivePolling::new(authorization.interval, runtime.clock.now());
    info_polling_started(&polling, authorization.expires_in);

    loop {
        ensure_polling_not_expired(runtime.clock, &polling, authorization.expires_in)
            .map_err(DeviceFlowRunError::Terminal)?;
        let decision = poll_once(runtime, request, authorization, &mut polling)?;
        if let Some(token) = apply_poll_decision(decision, &mut polling)? {
            return Ok(token);
        }
    }
}

fn ensure_polling_not_expired(
    clock: &dyn MonotonicClock,
    polling: &ActivePolling,
    expires_in: Duration,
) -> Result<(), TerminalDeviceFlowError> {
    let elapsed = polling.elapsed(clock.now());
    if has_expired(elapsed, polling.interval, expires_in) {
        info_polling_expired(elapsed, polling, expires_in);
        info_terminal_outcome(TerminalDeviceFlowError::ExpiredToken, polling.attempt);
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
    info!(attempt = polling.attempt, "metric.github_device_flow_poll_attempt_total");
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
        PollDecision::Failed(error) => fail_poll_decision(error, polling.attempt),
    }
}

#[derive(Clone, Copy, Debug)]
struct ActivePolling {
    interval: Duration,
    attempt: u64,
    started_at: Instant,
}

impl ActivePolling {
    const fn new(interval: Duration, started_at: Instant) -> Self {
        Self { interval, attempt: 0, started_at }
    }
    const fn record_attempt(&mut self) { self.attempt = self.attempt.saturating_add(1); }
    fn elapsed(&self, now: Instant) -> Duration { now.saturating_duration_since(self.started_at) }
    fn update_interval(&mut self, next_interval: Duration) {
        if next_interval > self.interval {
            info_interval_increase(self.attempt, self.interval, next_interval);
        }
        self.interval = next_interval;
    }
}

fn fail_poll_decision<O, S>(
    error: TerminalDeviceFlowError,
    attempt: u64,
) -> Result<Option<AccessToken>, DeviceFlowRunError<O, S>>
where
    O: std::error::Error + Send + Sync + 'static,
    S: std::error::Error + Send + Sync + 'static,
{
    info_terminal_outcome(error, attempt);
    Err(DeviceFlowRunError::Terminal(error))
}

fn info_polling_started(polling: &ActivePolling, expires_in: Duration) {
    info!(
        interval_seconds = polling.interval.as_secs(),
        expires_in_seconds = expires_in.as_secs(),
        "entering GitHub device-flow polling loop",
    );
}

fn info_polling_expired(elapsed: Duration, polling: &ActivePolling, expires_in: Duration) {
    info!(
        elapsed_seconds = elapsed.as_secs(),
        next_interval_seconds = polling.interval.as_secs(),
        expires_in_seconds = expires_in.as_secs(),
        "GitHub device-flow authorization expired before next poll",
    );
}

fn has_expired(elapsed: Duration, interval: Duration, expires_in: Duration) -> bool {
    elapsed.saturating_add(interval) >= expires_in
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
