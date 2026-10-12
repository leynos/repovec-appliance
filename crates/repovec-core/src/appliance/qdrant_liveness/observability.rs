//! Observability helpers for Qdrant liveness validation.

use tracing::Span;

use super::{QdrantLivenessConfig, QdrantLivenessError, QdrantLivenessReport};

/// Creates the tracing span shared by one runtime liveness check.
/// For example, the span records the endpoint and configured timeout for the check.
pub(super) fn qdrant_liveness_span(config: &QdrantLivenessConfig) -> Span {
    tracing::info_span!(
        "qdrant_liveness_check",
        endpoint = %config.endpoint(),
        timeout_ms = config.timeout().as_millis(),
    )
}

/// Records the success or failure outcome of one runtime liveness check.
/// For example, a report records success fields and a liveness error records its stable category.
pub(super) fn record_qdrant_liveness_result(
    span: &Span,
    config: &QdrantLivenessConfig,
    result: &Result<QdrantLivenessReport, QdrantLivenessError>,
) {
    match result {
        Ok(report) => record_qdrant_liveness_success(span, config, report),
        Err(error) => record_qdrant_liveness_failure(span, config, error),
    }
}

/// Records the successful check through its log and metric events.
/// For example, a successful report produces the success log and emits the success event.
fn record_qdrant_liveness_success(
    span: &Span,
    config: &QdrantLivenessConfig,
    report: &QdrantLivenessReport,
) {
    record_qdrant_liveness_success_log(span, config, report);
    record_qdrant_liveness_success_metric(span);
}

/// Emits the debug event for a successful Qdrant check.
/// For example, the event includes the endpoint, timeout, and returned server version.
fn record_qdrant_liveness_success_log(
    span: &Span,
    config: &QdrantLivenessConfig,
    report: &QdrantLivenessReport,
) {
    let endpoint = config.endpoint();
    let timeout_ms = config.timeout().as_millis();
    let version = report.version();
    tracing::debug!(
        parent: span,
        endpoint,
        timeout_ms,
        version,
        "Qdrant liveness check passed",
    );
}

/// Emits the success metric event within the liveness span.
/// For example, each successful check emits `metric.qdrant_liveness_success_total`.
fn record_qdrant_liveness_success_metric(span: &Span) {
    tracing::info!(
        parent: span,
        "metric.qdrant_liveness_success_total",
    );
}

/// Records a failed check through its log and metric events.
/// For example, the error is logged with its stable category and represented by the failure event.
fn record_qdrant_liveness_failure(
    span: &Span,
    config: &QdrantLivenessConfig,
    error: &QdrantLivenessError,
) {
    record_qdrant_liveness_failure_log(span, config, error);
    record_qdrant_liveness_failure_metric(span, error);
}

/// Emits the debug event for a failed Qdrant check.
/// For example, the event includes endpoint, timeout, error category, and error details.
fn record_qdrant_liveness_failure_log(
    span: &Span,
    config: &QdrantLivenessConfig,
    error: &QdrantLivenessError,
) {
    let endpoint = config.endpoint();
    let timeout_ms = config.timeout().as_millis();
    let error_category = qdrant_liveness_error_category(error);
    tracing::debug!(
        parent: span,
        endpoint,
        timeout_ms,
        error_category,
        error = %error,
        "Qdrant liveness check failed",
    );
}

/// Emits the failure metric event with a stable error category.
/// For example, a timeout is labelled `timeout` through `qdrant_liveness_error_category`.
fn record_qdrant_liveness_failure_metric(span: &Span, error: &QdrantLivenessError) {
    let error_category = qdrant_liveness_error_category(error);
    tracing::info!(
        parent: span,
        error_category,
        "metric.qdrant_liveness_failure_total",
    );
}

/// Returns a stable, non-secret category label for a liveness error.
///
/// # Examples
///
/// ```no_run
/// use std::time::Duration;
///
/// use repovec_core::appliance::qdrant_liveness::{
///     qdrant_liveness_error_category, QdrantLivenessError,
/// };
///
/// let error = QdrantLivenessError::Timeout {
///     timeout: Duration::from_secs(5),
/// };
///
/// assert_eq!(qdrant_liveness_error_category(&error), "timeout");
/// ```
#[must_use]
pub const fn qdrant_liveness_error_category(error: &QdrantLivenessError) -> &'static str {
    match error {
        QdrantLivenessError::MissingApiKeyFile { .. } => "missing_api_key_file",
        QdrantLivenessError::UnreadableApiKeyFile { .. } => "unreadable_api_key_file",
        QdrantLivenessError::EmptyApiKey => "empty_api_key",
        QdrantLivenessError::InvalidApiKey => "invalid_api_key",
        QdrantLivenessError::InvalidEndpoint { .. } => "invalid_endpoint",
        QdrantLivenessError::Timeout { .. } => "timeout",
        QdrantLivenessError::AuthenticationFailed => "authentication_failed",
        QdrantLivenessError::GrpcUnavailable { .. } => "grpc_unavailable",
        QdrantLivenessError::MissingServerVersion => "missing_server_version",
    }
}
