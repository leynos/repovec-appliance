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
