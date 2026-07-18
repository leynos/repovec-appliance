//! Observability helpers for Qdrant liveness validation.

use tracing::Span;

use super::{QdrantLivenessConfig, QdrantLivenessError, QdrantLivenessReport};

pub(super) fn qdrant_liveness_span(config: &QdrantLivenessConfig) -> Span {
    tracing::info_span!(
        "qdrant_liveness_check",
        endpoint = %config.endpoint(),
        timeout_ms = config.timeout().as_millis(),
    )
}

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

fn record_qdrant_liveness_success(
    span: &Span,
    config: &QdrantLivenessConfig,
    report: &QdrantLivenessReport,
) {
    record_qdrant_liveness_success_log(span, config, report);
    record_qdrant_liveness_success_metric(span);
}

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

fn record_qdrant_liveness_success_metric(span: &Span) {
    tracing::info!(
        parent: span,
        "metric.qdrant_liveness_success_total",
    );
}

fn record_qdrant_liveness_failure(
    span: &Span,
    config: &QdrantLivenessConfig,
    error: &QdrantLivenessError,
) {
    record_qdrant_liveness_failure_log(span, config, error);
    record_qdrant_liveness_failure_metric(span, error);
}

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
