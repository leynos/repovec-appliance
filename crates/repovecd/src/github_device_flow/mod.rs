//! Application orchestration for GitHub OAuth device-flow login.

use std::time::Duration;

use repovec_core::github_oauth::{
    AccessToken, ClientId, DeviceAuthorization, TerminalDeviceFlowError, TokenPollOutcome, UserCode,
};
use thiserror::Error;
use tracing::info;

mod observability;
mod polling;
mod runtime_debug;

pub use monotony::{MonotonicClock, StdMonotonicClock};
use observability::{device_flow_span, info_device_flow_result, info_device_flow_started};
use polling::poll_until_token;

/// OAuth API operations required by the device-flow use case.
#[doc = include_str!("examples/device_flow_api.rsdoc")]
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
#[doc = include_str!("examples/token_store.rsdoc")]
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
#[doc = include_str!("examples/sleeper.rsdoc")]
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
#[derive(Clone, Copy)]
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
    #[doc = include_str!("examples/runtime_new.rsdoc")]
    #[must_use]
    pub const fn new(api: &'a A, store: &'a T, sleeper: &'a S) -> Self {
        Self { api, store, sleeper, clock: &STD_MONOTONIC_CLOCK }
    }

    /// Creates a runtime adapter bundle with an explicit clock.
    #[doc = include_str!("examples/runtime_with_clock.rsdoc")]
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
#[doc = include_str!("examples/complete_device_flow.rsdoc")]
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
