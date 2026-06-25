//! Tracing helpers for the OAuth HTTP adapter.

use std::time::Instant;

use oauth2::reqwest::StatusCode;
use repovec_core::github_oauth::TokenPollOutcome;
use tracing::{Span, info_span};

pub(super) fn info_token_poll_outcome(span: &Span, outcome: &TokenPollOutcome) {
    let outcome_span = info_span!(
        parent: span,
        "metric.github_oauth_poll_outcome_total",
        outcome = token_poll_outcome_label(outcome),
    );
    let _outcome_entered = outcome_span.enter();
}

pub(super) fn info_http_request(span: &Span, status: StatusCode, started_at: Instant) {
    let request_span = info_span!(
        parent: span,
        "metric.github_oauth_http_request_total",
        status = status.as_u16(),
    );
    let _request_entered = request_span.enter();
    let duration_span = info_span!(
        parent: span,
        "metric.github_oauth_http_request_duration_ms",
        elapsed_ms = started_at.elapsed().as_millis(),
    );
    let _duration_entered = duration_span.enter();
}

pub(super) fn info_token_poll(span: &Span, status: StatusCode, started_at: Instant) {
    let poll_span = info_span!(
        parent: span,
        "metric.github_oauth_poll_total",
        status = status.as_u16(),
    );
    let _poll_entered = poll_span.enter();
    let duration_span = info_span!(
        parent: span,
        "metric.github_oauth_poll_duration_ms",
        elapsed_ms = started_at.elapsed().as_millis(),
    );
    let _duration_entered = duration_span.enter();
}

pub(super) fn info_adapter_failure(span: &Span, status: StatusCode) {
    let failure_span = info_span!(
        parent: span,
        "metric.github_oauth_adapter_failure_total",
        status = status.as_u16(),
    );
    let _failure_entered = failure_span.enter();
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
