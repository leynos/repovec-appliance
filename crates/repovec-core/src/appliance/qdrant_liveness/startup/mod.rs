//! Startup retry policy for Qdrant liveness checks.

use std::{future::Future, time::Duration};

use tokio::time::Instant;
use tracing::Span;

use super::{DEFAULT_QDRANT_GRPC_ENDPOINT, QdrantLivenessError, qdrant_liveness_error_category};

/// Bounded retry policy for daemon startup Qdrant liveness validation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct QdrantStartupLivenessPolicy {
    /// Maximum duration allowed for transient startup readiness failures.
    readiness_timeout: Duration,
    /// Delay between retryable startup liveness attempts.
    poll_interval: Duration,
    /// Endpoint attached to startup liveness tracing and metric events.
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

/// Records startup liveness failure and returns the last transient error, if any.
/// For example, expiry before the first probe returns `Timeout` using the policy readiness
/// duration.
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
/// Timing and tracing context shared by one startup liveness attempt.
#[derive(Clone, Copy)]
struct StartupLivenessAttempt<'a> {
    /// Tracing span that groups events for this startup validation.
    span: &'a Span,
    /// Instant when the startup readiness wait began.
    started_at: Instant,
    /// Deadline after which another liveness attempt cannot be made.
    deadline: Instant,
    /// Configured endpoint attached to attempt diagnostics.
    endpoint: &'a str,
    /// Readiness timeout budget captured for this startup attempt.
    readiness_timeout: Duration,
    /// One-based number of the current liveness attempt.
    attempt: u64,
}

impl StartupLivenessAttempt<'_> {
    /// Returns elapsed time since startup validation began, in milliseconds.
    /// For example, the value supports duration fields in attempt logs.
    fn elapsed_ms(&self) -> u128 { self.started_at.elapsed().as_millis() }

    /// Returns the configured readiness timeout in milliseconds.
    /// For example, the value is recorded alongside startup success and retry events.
    const fn readiness_timeout_ms(&self) -> u128 { self.readiness_timeout.as_millis() }
}
/// Internal outcome of one startup health check.
enum QdrantReadiness {
    /// The health check succeeded and Qdrant is ready.
    Ready,
    /// The health check failed with a transient error that may be retried.
    Retry(QdrantLivenessError),
}
/// Runs one health check and classifies its result for the startup retry loop.
/// For example, success becomes `Ready`, while an error is classified as retryable or returned
/// immediately.
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
/// Classifies a liveness error as retryable or permanent for startup.
/// For example, transient transport failures are recorded for retry; permanent configuration or
/// authentication failures are returned.
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
/// Returns whether an error must stop retries immediately or the deadline has elapsed.
/// For example, invalid endpoint configuration fails fast even when readiness time remains.
fn should_fail_qdrant_liveness_fast(error: &QdrantLivenessError, deadline: Instant) -> bool {
    is_permanent_qdrant_liveness_error(error) || Instant::now() >= deadline
}
/// Identifies errors that cannot be corrected by retrying the liveness request.
/// For example, missing key files, invalid endpoints, and authentication failures are permanent.
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
/// Marks the beginning of startup liveness validation in the tracing span.
/// For example, the supplied startup span receives a debug event before the first probe.
fn record_startup_liveness_started(span: &Span) {
    tracing::debug!(parent: span, "Qdrant startup liveness validation started");
}
/// Records successful startup validation in logs and the success metric.
/// For example, the successful attempt is logged with its count and elapsed duration.
fn record_startup_liveness_success(context: StartupLivenessAttempt<'_>) {
    record_startup_liveness_success_log(context);
    record_startup_liveness_success_metric(context);
}

/// Emits detailed success fields for the completed startup attempt.
/// For example, the log carries endpoint, attempt number, elapsed time, and readiness budget.
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

/// Emits the startup liveness success metric event.
/// For example, a ready Qdrant service records `metric.qdrant_startup_liveness_success_total`.
fn record_startup_liveness_success_metric(context: StartupLivenessAttempt<'_>) {
    let readiness_timeout_ms = context.readiness_timeout_ms();
    tracing::info!(
        parent: context.span,
        readiness_timeout_ms,
        "metric.qdrant_startup_liveness_success_total",
    );
}

/// Records a transient startup liveness error in logs and the retry metric.
/// For example, each failed attempt that remains within the readiness budget is counted as a retry.
fn record_startup_liveness_retry(context: StartupLivenessAttempt<'_>, error: &QdrantLivenessError) {
    record_startup_liveness_retry_log(context, error);
    record_startup_liveness_retry_metric(context, error);
}

/// Emits the debug event describing a retryable liveness failure.
/// For example, the event includes attempt timing and the stable error category.
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

/// Emits the startup retry metric with its error category.
/// For example, retry events distinguish transport failures from timeouts.
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

/// Emits the startup failure metric with error and timing context.
/// For example, a permanent error or exhausted readiness wait records one failure event.
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
