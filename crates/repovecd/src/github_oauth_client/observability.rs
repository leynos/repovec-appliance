//! Tracing helpers for the OAuth HTTP adapter.

use std::time::Instant;

use oauth2::reqwest::StatusCode;
use repovec_core::github_oauth::TokenPollOutcome;
use tracing::{Span, info};

/// Emits the classified outcome for a token poll on its request span.
/// For example, `SlowDown` records the non-secret label `slow_down`.
pub(super) fn info_token_poll_outcome(span: &Span, outcome: &TokenPollOutcome) {
    info!(
        parent: span,
        outcome = token_poll_outcome_label(outcome),
        "metric.github_oauth_poll_outcome_total",
    );
}

/// Emits status and elapsed-time events for a device-code HTTP request.
/// For example, an `OK` response records status `200` and its duration.
pub(super) fn info_http_request(span: &Span, status: StatusCode, started_at: Instant) {
    info_status(span, "metric.github_oauth_http_request_total", status);
    info_duration(span, "metric.github_oauth_http_request_duration_ms", started_at);
}

/// Emits status and elapsed-time events for a token-poll HTTP request.
/// For example, an `ACCEPTED` response records status `202` and its duration.
pub(super) fn info_token_poll(span: &Span, status: StatusCode, started_at: Instant) {
    info_status(span, "metric.github_oauth_poll_total", status);
    info_duration(span, "metric.github_oauth_poll_duration_ms", started_at);
}

/// Emits the HTTP status associated with an adapter-level failure.
/// For example, a `BAD_GATEWAY` response records status `502`.
pub(super) fn info_adapter_failure(span: &Span, status: StatusCode) {
    info_status(span, "metric.github_oauth_adapter_failure_total", status);
}

/// Emits one status metric event beneath the supplied span.
/// For example, `StatusCode::OK` records the numeric status `200`.
fn info_status(span: &Span, metric: &'static str, status: StatusCode) {
    info!(
        parent: span,
        status = status.as_u16(),
        "{metric}",
    );
}

/// Emits elapsed milliseconds since the request start time.
/// For example, a request started one millisecond ago records about `1`.
fn info_duration(span: &Span, metric: &'static str, started_at: Instant) {
    info!(
        parent: span,
        elapsed_ms = started_at.elapsed().as_millis(),
        "{metric}",
    );
}

/// Returns a stable, non-secret label for a token-poll outcome.
/// For example, `SlowDown` maps to `slow_down`.
const fn token_poll_outcome_label(outcome: &TokenPollOutcome) -> &'static str {
    match outcome {
        TokenPollOutcome::Authorized(_) => "authorized",
        TokenPollOutcome::AuthorizationPending => "pending",
        TokenPollOutcome::SlowDown => "slow_down",
        TokenPollOutcome::AccessDenied => "access_denied",
        TokenPollOutcome::ExpiredToken => "expired_token",
    }
}

#[cfg(test)]
mod tests {
    //! Tests for OAuth HTTP adapter observability events.

    use super::*;
    use crate::tracing_test::capture_info_logs;

    #[test]
    fn token_poll_outcome_metric_is_emitted_as_an_event() {
        let span = tracing::info_span!("github_oauth_client.test");
        let ((), logs) = capture_info_logs(|| {
            info_token_poll_outcome(&span, &TokenPollOutcome::SlowDown);
        })
        .expect("capturing tracing logs should succeed");

        assert!(logs.contains("metric.github_oauth_poll_outcome_total"));
        assert!(logs.contains("slow_down"));
    }

    #[test]
    fn device_code_request_metrics_are_emitted_as_events() {
        let span = tracing::info_span!("github_oauth_client.test");
        let ((), logs) = capture_info_logs(|| {
            info_http_request(&span, StatusCode::OK, Instant::now());
        })
        .expect("capturing tracing logs should succeed");

        assert!(logs.contains("metric.github_oauth_http_request_total"));
        assert!(logs.contains("status=200"));
        assert!(logs.contains("metric.github_oauth_http_request_duration_ms"));
        assert!(logs.contains("elapsed_ms="));
    }

    #[test]
    fn token_poll_metrics_are_emitted_as_events() {
        let span = tracing::info_span!("github_oauth_client.test");
        let ((), logs) = capture_info_logs(|| {
            info_token_poll(&span, StatusCode::ACCEPTED, Instant::now());
        })
        .expect("capturing tracing logs should succeed");

        assert!(logs.contains("metric.github_oauth_poll_total"));
        assert!(logs.contains("status=202"));
        assert!(logs.contains("metric.github_oauth_poll_duration_ms"));
        assert!(logs.contains("elapsed_ms="));
    }

    #[test]
    fn adapter_failure_metric_is_emitted_as_an_event() {
        let span = tracing::info_span!("github_oauth_client.test");
        let ((), logs) = capture_info_logs(|| {
            info_adapter_failure(&span, StatusCode::BAD_GATEWAY);
        })
        .expect("capturing tracing logs should succeed");

        assert!(logs.contains("metric.github_oauth_adapter_failure_total"));
        assert!(logs.contains("status=502"));
    }
}
