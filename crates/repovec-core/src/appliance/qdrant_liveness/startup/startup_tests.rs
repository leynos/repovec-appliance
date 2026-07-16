//! Unit coverage for startup retry classification and observability.

use std::{cell::Cell, time::Duration};

use super::QdrantStartupLivenessPolicy;
use crate::appliance::qdrant_liveness::QdrantLivenessError;

#[test]
fn startup_liveness_retry_logs_attempt_elapsed_and_metric() -> Result<(), String> {
    let attempts = Cell::new(0);
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_time()
        .build()
        .expect("test runtime should build");
    let policy =
        QdrantStartupLivenessPolicy::new(Duration::from_millis(50), Duration::from_millis(1));

    let (result, logs) = repovec_test_helpers::capture_logs(|| {
        runtime.block_on(super::wait_for_qdrant_startup_liveness(
            || {
                let attempt = attempts.get();
                attempts.set(attempt + 1);
                std::future::ready(transient_qdrant_result(attempt))
            },
            policy,
        ))
    })?;

    repovec_test_helpers::ensure(result.is_ok(), "transient retry should eventually pass")?;
    repovec_test_helpers::ensure_log_line_contains(
        &logs,
        "DEBUG",
        "attempt=1",
        "retry log should include the attempt number",
    )?;
    repovec_test_helpers::ensure_log_line_contains(
        &logs,
        "DEBUG",
        "elapsed_ms=",
        "retry log should include elapsed time",
    )?;
    repovec_test_helpers::ensure_log_line_contains(
        &logs,
        "DEBUG",
        "endpoint=\"http://127.0.0.1:6334\"",
        "retry log should include the Qdrant endpoint",
    )?;
    repovec_test_helpers::ensure_log_line_contains(
        &logs,
        "DEBUG",
        "readiness_timeout_ms=50",
        "retry log should include the readiness timeout",
    )?;
    repovec_test_helpers::ensure_log_line_contains(
        &logs,
        "DEBUG",
        "error_category=\"grpc_unavailable\"",
        "retry log should include the liveness error category",
    )?;
    repovec_test_helpers::ensure_log_line_contains(
        &logs,
        "INFO",
        "metric.qdrant_startup_liveness_retry_total",
        "retry should emit a bounded metric event",
    )
}

#[test]
fn startup_liveness_success_logs_a_bounded_metric() -> Result<(), String> {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_time()
        .build()
        .expect("test runtime should build");
    let policy =
        QdrantStartupLivenessPolicy::new(Duration::from_millis(50), Duration::from_millis(1));

    let (result, logs) = repovec_test_helpers::capture_logs(|| {
        runtime.block_on(super::wait_for_qdrant_startup_liveness(|| async { Ok(()) }, policy))
    })?;

    repovec_test_helpers::ensure(result.is_ok(), "successful liveness check should pass")?;
    repovec_test_helpers::ensure_log_line_contains(
        &logs,
        "DEBUG",
        "Qdrant startup liveness validated",
        "success should emit a liveness log",
    )?;
    repovec_test_helpers::ensure_log_line_contains(
        &logs,
        "INFO",
        "metric.qdrant_startup_liveness_success_total",
        "success should emit a bounded metric event",
    )
}

#[test]
fn startup_liveness_returns_last_transient_error_at_the_deadline() {
    let attempts = Cell::new(0);
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_time()
        .build()
        .expect("test runtime should build");
    let policy =
        QdrantStartupLivenessPolicy::new(Duration::from_millis(10), Duration::from_secs(1));

    let result = runtime.block_on(super::wait_for_qdrant_startup_liveness(
        || {
            let attempt = attempts.get();
            attempts.set(attempt + 1);
            std::future::ready(transient_qdrant_result(attempt))
        },
        policy,
    ));

    assert!(matches!(result, Err(QdrantLivenessError::GrpcUnavailable { .. })));
    assert_eq!(attempts.get(), 1);
}

#[test]
fn startup_liveness_rejects_success_after_the_deadline() {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_time()
        .build()
        .expect("test runtime should build");
    let policy =
        QdrantStartupLivenessPolicy::new(Duration::from_millis(10), Duration::from_secs(1));

    let result = runtime.block_on(super::wait_for_qdrant_startup_liveness(
        || async {
            tokio::time::sleep(Duration::from_millis(50)).await;
            Ok(())
        },
        policy,
    ));

    assert!(matches!(result, Err(QdrantLivenessError::Timeout { .. })));
}

fn transient_qdrant_result(attempt: i32) -> Result<(), QdrantLivenessError> {
    match attempt {
        0 => Err(QdrantLivenessError::GrpcUnavailable {
            message: String::from("connection refused"),
        }),
        _ => Ok(()),
    }
}
