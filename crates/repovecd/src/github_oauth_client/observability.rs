//! Tracing helpers for the OAuth HTTP adapter.

use std::time::Instant;

use oauth2::reqwest::StatusCode;
use repovec_core::github_oauth::TokenPollOutcome;
use tracing::{Span, info};

pub(super) fn info_token_poll_outcome(span: &Span, outcome: &TokenPollOutcome) {
    info!(
        parent: span,
        outcome = token_poll_outcome_label(outcome),
        "metric.github_oauth_poll_outcome_total",
    );
}

pub(super) fn info_http_request(span: &Span, status: StatusCode, started_at: Instant) {
    info_status(span, "metric.github_oauth_http_request_total", status);
    info_duration(span, "metric.github_oauth_http_request_duration_ms", started_at);
}

pub(super) fn info_token_poll(span: &Span, status: StatusCode, started_at: Instant) {
    info_status(span, "metric.github_oauth_poll_total", status);
    info_duration(span, "metric.github_oauth_poll_duration_ms", started_at);
}

pub(super) fn info_adapter_failure(span: &Span, status: StatusCode) {
    info_status(span, "metric.github_oauth_adapter_failure_total", status);
}

fn info_status(span: &Span, metric: &'static str, status: StatusCode) {
    info!(
        parent: span,
        status = status.as_u16(),
        "{metric}",
    );
}

fn info_duration(span: &Span, metric: &'static str, started_at: Instant) {
    info!(
        parent: span,
        elapsed_ms = started_at.elapsed().as_millis(),
        "{metric}",
    );
}

const fn token_poll_outcome_label(outcome: &TokenPollOutcome) -> &'static str {
    match outcome {
        TokenPollOutcome::Authorized(_) => "authorized",
        TokenPollOutcome::AuthorizationPending => "pending",
        TokenPollOutcome::SlowDown => "slow_down",
        TokenPollOutcome::AccessDenied => "access_denied",
        TokenPollOutcome::ExpiredToken => "expired_token",
    }
}
