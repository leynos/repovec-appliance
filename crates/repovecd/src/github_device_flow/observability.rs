//! Tracing helpers for GitHub device-flow orchestration.

use std::time::Duration;

use repovec_core::github_oauth::TerminalDeviceFlowError;
use tracing::{Span, info, info_span};

use super::{CompletedDeviceFlow, DeviceFlowLoginRequest, DeviceFlowRunError};

pub(super) fn device_flow_span(request: &DeviceFlowLoginRequest) -> Span {
    info_span!("github_device_flow.complete", scope_count = request.scopes.len())
}

pub(super) fn info_device_flow_started() {
    info!("metric.github_device_flow_started_total");
}

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

fn info_failure(kind: &'static str, error: &dyn std::error::Error) {
    info!(
        error = %error,
        failure_kind = kind,
        "metric.github_device_flow_failed_total",
    );
}

pub(super) fn info_terminal_outcome(error: TerminalDeviceFlowError, attempt: u64) {
    let outcome = terminal_error_label(error);
    info_terminal_context(attempt, outcome);
    info_terminal_total(outcome);
}

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

fn info_terminal_context(attempt: u64, outcome: &'static str) {
    info!("github_device_flow.terminal_outcome attempt={attempt} outcome={outcome}");
}

fn info_terminal_total(outcome: &'static str) {
    info!("metric.github_device_flow_terminal_total outcome={outcome}");
}

fn info_slow_down(attempt: u64, previous_interval_seconds: u64, next_interval_seconds: u64) {
    info!(
        "github_device_flow.slow_down attempt={attempt} \
         previous_interval_seconds={previous_interval_seconds} \
         next_interval_seconds={next_interval_seconds}"
    );
}

fn info_slow_down_total() {
    info!("metric.github_device_flow_slow_down_total");
}

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
