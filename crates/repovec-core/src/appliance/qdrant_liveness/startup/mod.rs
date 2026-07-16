//! Startup retry policy for Qdrant liveness checks.

use std::{future::Future, time::Duration};

use tokio::time::Instant;
use tracing::Span;

use super::{DEFAULT_QDRANT_GRPC_ENDPOINT, QdrantLivenessError, qdrant_liveness_error_category};

/// Bounded retry policy for daemon startup Qdrant liveness validation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct QdrantStartupLivenessPolicy {
    readiness_timeout: Duration,
    poll_interval: Duration,
    endpoint: &'static str,
}

impl QdrantStartupLivenessPolicy {
    /// Creates a startup liveness policy from explicit timing values.
    #[must_use]
    pub const fn new(readiness_timeout: Duration, poll_interval: Duration) -> Self {
        Self { readiness_timeout, poll_interval, endpoint: DEFAULT_QDRANT_GRPC_ENDPOINT }
    }

    /// Returns the maximum time spent waiting for transient readiness failures.
    #[must_use]
    pub const fn readiness_timeout(&self) -> Duration { self.readiness_timeout }

    /// Returns the delay between retryable liveness attempts.
    #[must_use]
    pub const fn poll_interval(&self) -> Duration { self.poll_interval }

    /// Returns the endpoint described by startup liveness observability.
    #[must_use]
    pub const fn endpoint(&self) -> &str { self.endpoint }
}

/// Waits for Qdrant liveness while failing permanent configuration errors fast.
///
/// The supplied `health_check` closure is the adapter boundary. It may perform
/// filesystem, endpoint, or transport work, while this function only owns the
/// startup retry policy and observability contract.
///
/// # Errors
///
/// Returns the first permanent liveness error immediately, or the last
/// transient liveness error after the configured readiness timeout elapses.
pub async fn wait_for_qdrant_startup_liveness<H, F>(
    mut health_check: H,
    policy: QdrantStartupLivenessPolicy,
) -> Result<(), QdrantLivenessError>
where
    H: FnMut() -> F,
    F: Future<Output = Result<(), QdrantLivenessError>>,
{
    let started_at = Instant::now();
    let deadline = started_at + policy.readiness_timeout();
    let span = tracing::info_span!(
        "qdrant_startup_liveness",
        endpoint = policy.endpoint(),
        readiness_timeout_ms = policy.readiness_timeout().as_millis(),
        poll_interval_ms = policy.poll_interval().as_millis(),
    );
    let mut attempt = 1_u64;
    let mut last_transient_error = None;

    record_startup_liveness_started(&span);
    loop {
        let context = StartupLivenessAttempt {
            span: &span,
            started_at,
            deadline,
            endpoint: policy.endpoint(),
            readiness_timeout: policy.readiness_timeout(),
            attempt,
        };
        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            return record_startup_liveness_timeout(context, last_transient_error, policy);
        }

        match tokio::time::timeout(
            remaining,
            check_qdrant_liveness_attempt(&mut health_check, context),
        )
        .await
        {
            Ok(Ok(QdrantReadiness::Ready)) => {
                record_startup_liveness_success(context);
                return Ok(());
            }
            Ok(Ok(QdrantReadiness::Retry(error))) => {
                last_transient_error = Some(error);
                attempt += 1;
                let remaining_sleep = deadline.saturating_duration_since(Instant::now());
                if remaining_sleep.is_zero() {
                    return record_startup_liveness_timeout(context, last_transient_error, policy);
                }
                tokio::time::sleep(policy.poll_interval().min(remaining_sleep)).await;
            }
            Ok(Err(error)) => {
                record_startup_liveness_failure(context, &error);
                return Err(error);
            }
            Err(_) => {
                return record_startup_liveness_timeout(context, last_transient_error, policy);
            }
        }
    }
}

fn record_startup_liveness_timeout(
    context: StartupLivenessAttempt<'_>,
    last_transient_error: Option<QdrantLivenessError>,
    policy: QdrantStartupLivenessPolicy,
) -> Result<(), QdrantLivenessError> {
    let error = last_transient_error
        .unwrap_or_else(|| QdrantLivenessError::Timeout { timeout: policy.readiness_timeout() });
    record_startup_liveness_failure(context, &error);
    Err(error)
}
#[derive(Clone, Copy)]
struct StartupLivenessAttempt<'a> {
    span: &'a Span,
    started_at: Instant,
    deadline: Instant,
    endpoint: &'a str,
    readiness_timeout: Duration,
    attempt: u64,
}

impl StartupLivenessAttempt<'_> {
    fn elapsed_ms(&self) -> u128 { self.started_at.elapsed().as_millis() }

    const fn readiness_timeout_ms(&self) -> u128 { self.readiness_timeout.as_millis() }
}
enum QdrantReadiness {
    Ready,
    Retry(QdrantLivenessError),
}
async fn check_qdrant_liveness_attempt<H, F>(
    health_check: &mut H,
    context: StartupLivenessAttempt<'_>,
) -> Result<QdrantReadiness, QdrantLivenessError>
where
    H: FnMut() -> F,
    F: Future<Output = Result<(), QdrantLivenessError>>,
{
    health_check()
        .await
        .map(|()| QdrantReadiness::Ready)
        .or_else(|error| classify_qdrant_liveness_error(error, context))
}
fn classify_qdrant_liveness_error(
    error: QdrantLivenessError,
    context: StartupLivenessAttempt<'_>,
) -> Result<QdrantReadiness, QdrantLivenessError> {
    if should_fail_qdrant_liveness_fast(&error, context.deadline) {
        Err(error)
    } else {
        record_startup_liveness_retry(context, &error);
        Ok(QdrantReadiness::Retry(error))
    }
}
fn should_fail_qdrant_liveness_fast(error: &QdrantLivenessError, deadline: Instant) -> bool {
    is_permanent_qdrant_liveness_error(error) || Instant::now() >= deadline
}
const fn is_permanent_qdrant_liveness_error(error: &QdrantLivenessError) -> bool {
    matches!(
        error,
        QdrantLivenessError::MissingApiKeyFile { .. }
            | QdrantLivenessError::UnreadableApiKeyFile { .. }
            | QdrantLivenessError::EmptyApiKey
            | QdrantLivenessError::InvalidApiKey
            | QdrantLivenessError::InvalidEndpoint { .. }
            | QdrantLivenessError::AuthenticationFailed
            | QdrantLivenessError::MissingServerVersion
    )
}
fn record_startup_liveness_started(span: &Span) {
    tracing::debug!(parent: span, "Qdrant startup liveness validation started");
}
fn record_startup_liveness_success(context: StartupLivenessAttempt<'_>) {
    record_startup_liveness_success_log(context);
    record_startup_liveness_success_metric(context);
}

fn record_startup_liveness_success_log(context: StartupLivenessAttempt<'_>) {
    let attempt = context.attempt;
    let endpoint = context.endpoint;
    let readiness_timeout_ms = context.readiness_timeout_ms();
    let elapsed_ms = context.elapsed_ms();
    tracing::debug!(
        parent: context.span,
        endpoint,
        readiness_timeout_ms,
        attempt,
        elapsed_ms,
        "Qdrant startup liveness validated",
    );
}

fn record_startup_liveness_success_metric(context: StartupLivenessAttempt<'_>) {
    let readiness_timeout_ms = context.readiness_timeout_ms();
    tracing::info!(
        parent: context.span,
        readiness_timeout_ms,
        "metric.qdrant_startup_liveness_success_total",
    );
}

fn record_startup_liveness_retry(context: StartupLivenessAttempt<'_>, error: &QdrantLivenessError) {
    record_startup_liveness_retry_log(context, error);
    record_startup_liveness_retry_metric(context, error);
}

fn record_startup_liveness_retry_log(
    context: StartupLivenessAttempt<'_>,
    error: &QdrantLivenessError,
) {
    let attempt = context.attempt;
    let endpoint = context.endpoint;
    let readiness_timeout_ms = context.readiness_timeout_ms();
    let elapsed_ms = context.elapsed_ms();
    let error_category = qdrant_liveness_error_category(error);
    tracing::debug!(
        parent: context.span,
        endpoint,
        readiness_timeout_ms,
        attempt,
        elapsed_ms,
        error_category,
        error = %error,
        "Qdrant liveness not ready; retrying",
    );
}

fn record_startup_liveness_retry_metric(
    context: StartupLivenessAttempt<'_>,
    error: &QdrantLivenessError,
) {
    let attempt = context.attempt;
    let readiness_timeout_ms = context.readiness_timeout_ms();
    let error_category = qdrant_liveness_error_category(error);
    tracing::info!(
        parent: context.span,
        readiness_timeout_ms,
        attempt,
        error_category,
        "metric.qdrant_startup_liveness_retry_total",
    );
}

fn record_startup_liveness_failure(
    context: StartupLivenessAttempt<'_>,
    error: &QdrantLivenessError,
) {
    let readiness_timeout_ms = context.readiness_timeout_ms();
    let error_category = qdrant_liveness_error_category(error);
    tracing::info!(
        parent: context.span,
        readiness_timeout_ms,
        attempt = context.attempt,
        elapsed_ms = context.elapsed_ms(),
        error_category,
        error = %error,
        "metric.qdrant_startup_liveness_failure_total",
    );
}

#[cfg(test)]
mod startup_tests;
