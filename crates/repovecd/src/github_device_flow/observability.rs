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
    if result.is_ok() {
        info!("metric.github_device_flow_completed_total");
    }
}

pub(super) fn info_terminal_outcome(error: TerminalDeviceFlowError, attempt: u64) {
    let outcome = terminal_error_label(error);
    let terminal_span = info_span!("github_device_flow.terminal_outcome", attempt, outcome);
    let _terminal_entered = terminal_span.enter();
    let metric_span = info_span!("metric.github_device_flow_terminal_total", outcome);
    let _metric_entered = metric_span.enter();
}

pub(super) fn info_interval_increase(
    attempt: u64,
    previous_interval: Duration,
    next_interval: Duration,
) {
    let slow_down_span = info_span!(
        "github_device_flow.slow_down",
        attempt,
        previous_interval_seconds = previous_interval.as_secs(),
        next_interval_seconds = next_interval.as_secs()
    );
    let _slow_down_entered = slow_down_span.enter();
    let metric_span = info_span!("metric.github_device_flow_slow_down_total");
    let _metric_entered = metric_span.enter();
}

const fn terminal_error_label(error: TerminalDeviceFlowError) -> &'static str {
    match error {
        TerminalDeviceFlowError::AccessDenied => "access_denied",
        TerminalDeviceFlowError::ExpiredToken => "expired_token",
    }
}
