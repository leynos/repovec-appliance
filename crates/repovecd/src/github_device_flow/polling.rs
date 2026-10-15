//! Device-flow polling state, expiry decisions, and terminal outcomes.

use std::time::{Duration, Instant};

use repovec_core::github_oauth::{
    AccessToken, DeviceAuthorization, PollDecision, TerminalDeviceFlowError, apply_poll_outcome,
};
use tracing::info;

use super::{
    DeviceFlowApi, DeviceFlowLoginRequest, DeviceFlowRunError, DeviceFlowRuntime, MonotonicClock,
    Sleeper, TokenStore,
    observability::{info_interval_increase, info_terminal_outcome},
};

/// Polls GitHub until authorization completes or a terminal outcome occurs.
///
/// If the first response asks the client to continue and the second completes,
/// this returns the token after two attempts; expiry and adapter failures remain
/// typed errors.
pub(super) fn poll_until_token<A, T, S>(
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

/// Rejects a next poll that would reach or pass the authorization deadline.
///
/// With 56 seconds elapsed, a 5-second interval, and a 60-second lifetime,
/// this returns `ExpiredToken` because the next attempt would cross expiry.
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

/// Sleeps for one interval, counts the attempt, and calls the token endpoint.
///
/// With a 5-second interval, one call sleeps for 5 seconds and records attempt
/// 1 before returning the API outcome or OAuth error.
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

/// Applies one protocol decision to polling state and returns any completed token.
///
/// A `Continue` decision updates the interval and returns `None`; `Complete`
/// returns `Some(token)`, while `Failed` becomes a terminal error.
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

/// Tracks the interval, attempt count, and monotonic start of one authorization.
#[derive(Clone, Copy, Debug)]
struct ActivePolling {
    /// Delay to wait before the next token request.
    interval: Duration,
    /// Number of token requests started so far.
    attempt: u64,
    /// Monotonic instant used to measure the authorization lifetime.
    started_at: Instant,
}

impl ActivePolling {
    /// Starts polling at `interval` with no attempts recorded yet.
    ///
    /// For example, a 5-second interval starts with attempt 0.
    const fn new(interval: Duration, started_at: Instant) -> Self {
        Self { interval, attempt: 0, started_at }
    }
    /// Records another token request without allowing the count to wrap.
    ///
    /// Starting at 0, one call records attempt 1.
    const fn record_attempt(&mut self) { self.attempt = self.attempt.saturating_add(1); }
    /// Measures elapsed time from the start instant using monotonic time.
    ///
    /// If `now` is 3 seconds after the start, the result is 3 seconds.
    fn elapsed(&self, now: Instant) -> Duration { now.saturating_duration_since(self.started_at) }
    /// Updates the next delay and reports a server-requested slowdown increase.
    ///
    /// A change from 5 to 10 seconds emits a slowdown event; 10 to 5 does not.
    fn update_interval(&mut self, next_interval: Duration) {
        if next_interval > self.interval {
            info_interval_increase(self.attempt, self.interval, next_interval);
        }
        self.interval = next_interval;
    }
}

/// Emits the terminal outcome and returns it as a typed device-flow error.
///
/// For example, an `AccessDenied` decision emits the matching outcome label
/// with the current attempt count and returns `DeviceFlowRunError::Terminal`.
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

/// Records the initial interval and authorization lifetime when polling begins.
///
/// For a 5-second interval and a 900-second lifetime, the event carries those
/// two values in seconds.
fn info_polling_started(polling: &ActivePolling, expires_in: Duration) {
    info!(
        interval_seconds = polling.interval.as_secs(),
        expires_in_seconds = expires_in.as_secs(),
        "entering GitHub device-flow polling loop",
    );
}

/// Records elapsed time and the next interval when authorization expires.
///
/// For 56 seconds elapsed, a 5-second next interval, and a 60-second lifetime,
/// the event reports 56, 5, and 60 seconds respectively.
fn info_polling_expired(elapsed: Duration, polling: &ActivePolling, expires_in: Duration) {
    info!(
        elapsed_seconds = elapsed.as_secs(),
        next_interval_seconds = polling.interval.as_secs(),
        expires_in_seconds = expires_in.as_secs(),
        "GitHub device-flow authorization expired before next poll",
    );
}

/// Checks whether waiting one more interval reaches the authorization deadline.
///
/// At 56 seconds elapsed with a 5-second interval and 60-second lifetime, the
/// result is `true` because the next poll would begin after expiry.
fn has_expired(elapsed: Duration, interval: Duration, expires_in: Duration) -> bool {
    elapsed.saturating_add(interval) >= expires_in
}
