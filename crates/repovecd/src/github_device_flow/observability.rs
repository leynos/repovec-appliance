//! Tracing helpers for GitHub device-flow orchestration.

use std::time::Duration;

use repovec_core::github_oauth::TerminalDeviceFlowError;
use tracing::{Span, info, info_span};

use super::{CompletedDeviceFlow, DeviceFlowLoginRequest, DeviceFlowRunError};

/// Creates the span that groups one login attempt and its scope count.
///
/// A request with two scopes enters `github_device_flow.complete` with
/// `scope_count=2`.
pub(super) fn device_flow_span(request: &DeviceFlowLoginRequest) -> Span {
    info_span!("github_device_flow.complete", scope_count = request.scopes.len())
}

/// Records the start counter for a device-flow login.
///
/// When a login begins, this emits `metric.github_device_flow_started_total`.
pub(super) fn info_device_flow_started() {
    info!("metric.github_device_flow_started_total");
}

/// Records completion or the adapter category for a finished login attempt.
///
/// An OAuth error emits the failure metric with `failure_kind=oauth`; a
/// successful result emits the completion metric.
pub(super) fn info_device_flow_result<O, S>(
    result: &Result<CompletedDeviceFlow, DeviceFlowRunError<O, S>>,
) where
    O: std::error::Error + Send + Sync + 'static,
    S: std::error::Error + Send + Sync + 'static,
{
    match result {
        Ok(_) => info!("metric.github_device_flow_completed_total"),
        Err(DeviceFlowRunError::OAuth(error)) => info_failure("oauth", error),
        Err(DeviceFlowRunError::Storage(error)) => info_failure("storage", error),
        Err(DeviceFlowRunError::Terminal(_)) => {}
    }
}

/// Records a failure event tagged with the supplied adapter category.
///
/// For an OAuth adapter failure, `kind` is `oauth` and the failure counter is
/// emitted once.
fn info_failure(kind: &'static str, error: &dyn std::error::Error) {
    info!(
        error = %error,
        failure_kind = kind,
        "metric.github_device_flow_failed_total",
    );
}

/// Records the protocol outcome and total for a terminal polling result.
///
/// An access-denied response on attempt 3 emits the `access_denied` label and
/// records attempt 3 in the outcome event.
pub(super) fn info_terminal_outcome(error: TerminalDeviceFlowError, attempt: u64) {
    let outcome = terminal_error_label(error);
    info_terminal_context(attempt, outcome);
    info_terminal_total(outcome);
}

/// Records when GitHub asks the client to increase its polling interval.
///
/// An increase from 5 to 10 seconds on attempt 2 emits the slowdown event and
/// increments the slowdown counter.
pub(super) fn info_interval_increase(
    attempt: u64,
    previous_interval: Duration,
    next_interval: Duration,
) {
    let previous_interval_seconds = previous_interval.as_secs();
    let next_interval_seconds = next_interval.as_secs();
    info_slow_down(attempt, previous_interval_seconds, next_interval_seconds);
    info_slow_down_total();
}

/// Emits the attempt number and stable label for a terminal outcome.
///
/// Attempt 3 with `access_denied` produces a context event carrying those
/// values.
fn info_terminal_context(attempt: u64, outcome: &'static str) {
    info!("github_device_flow.terminal_outcome attempt={attempt} outcome={outcome}");
}

/// Increments the terminal-outcome counter for its stable label.
///
/// An `expired_token` result emits the terminal counter with that label.
fn info_terminal_total(outcome: &'static str) {
    info!("metric.github_device_flow_terminal_total outcome={outcome}");
}

/// Emits one interval increase with its attempt and before/after values.
///
/// An increase on attempt 2 from 5 to 10 seconds reports those three values.
fn info_slow_down(attempt: u64, previous_interval_seconds: u64, next_interval_seconds: u64) {
    info!(
        "github_device_flow.slow_down attempt={attempt} \
         previous_interval_seconds={previous_interval_seconds} \
         next_interval_seconds={next_interval_seconds}"
    );
}

/// Increments the counter for server-requested polling slowdowns.
///
/// After an interval increase, this emits
/// `metric.github_device_flow_slow_down_total`.
fn info_slow_down_total() {
    info!("metric.github_device_flow_slow_down_total");
}

/// Maps a terminal protocol error to its stable metric label.
///
/// `AccessDenied` maps to `access_denied`, and `ExpiredToken` maps to
/// `expired_token`.
const fn terminal_error_label(error: TerminalDeviceFlowError) -> &'static str {
    match error {
        TerminalDeviceFlowError::AccessDenied => "access_denied",
        TerminalDeviceFlowError::ExpiredToken => "expired_token",
    }
}

#[cfg(test)]
mod tests {
    //! Tests for device-flow observability events.

    use std::time::Duration;

    use thiserror::Error;

    use super::*;
    use crate::tracing_test::capture_info_logs;

    #[test]
    fn slow_down_metric_is_emitted_as_an_event() {
        let ((), logs) = capture_info_logs(|| {
            info_interval_increase(2, Duration::from_secs(5), Duration::from_secs(10));
        })
        .expect("capturing tracing logs should succeed");

        assert!(logs.contains("github_device_flow.slow_down"));
        assert!(logs.contains("metric.github_device_flow_slow_down_total"));
        assert!(logs.contains("next_interval_seconds=10"));
    }

    #[test]
    fn oauth_failure_metric_is_emitted_as_an_event() {
        let result = Err(DeviceFlowRunError::<FakeError, FakeError>::OAuth(FakeError));

        let ((), logs) = capture_info_logs(|| info_device_flow_result(&result))
            .expect("capturing tracing logs should succeed");

        assert!(logs.contains("metric.github_device_flow_failed_total"));
        assert!(logs.contains("oauth"));
    }

    #[derive(Debug, Error)]
    #[error("fake failure")]
    struct FakeError;
}
