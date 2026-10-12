//! Unit coverage for shared daemon startup checks.

use std::{
    cell::{Cell, RefCell},
    time::Duration,
};

use rstest::rstest;

use super::{
    DaemonStartupError, QdrantLivenessError, STARTUP_FAILURE_EXIT_CODE, SystemdUnitError,
    log_daemon_startup_error, validate_daemon_startup_contracts_with,
    validate_qdrant_liveness_with_policy,
};

#[test]
fn startup_succeeds_when_validation_passes() {
    validate_daemon_startup_contracts_with(|| Ok(()), || async { Ok(()) })
        .expect("startup contracts should pass");
}

#[test]
fn startup_logs_successful_validation() -> Result<(), String> {
    let (result, logs) = repovec_test_helpers::capture_logs(|| {
        validate_daemon_startup_contracts_with(|| Ok(()), || async { Ok(()) })
    })?;

    repovec_test_helpers::ensure(result.is_ok(), "startup should pass")?;
    repovec_test_helpers::ensure_log_line_contains(
        &logs,
        "DEBUG",
        "systemd unit contract validated at daemon startup",
        "startup should log systemd validation success",
    )?;
    repovec_test_helpers::ensure_log_line_contains(
        &logs,
        "DEBUG",
        "Qdrant liveness validated",
        "startup should log Qdrant validation success",
    )
}

#[test]
fn startup_runs_real_systemd_validation() -> Result<(), String> {
    let (result, logs) = repovec_test_helpers::capture_logs(|| {
        validate_daemon_startup_contracts_with(
            super::validate_and_trace_checked_in_units,
            || async { Ok(()) },
        )
    })?;

    repovec_test_helpers::ensure(
        result.is_ok(),
        "checked-in units and injected Qdrant liveness should pass",
    )?;
    repovec_test_helpers::ensure_log_line_contains(
        &logs,
        "TRACE",
        "systemd unit contract validated",
        "startup should call the real systemd validator",
    )
}

#[test]
fn startup_skips_qdrant_after_systemd_failure() {
    let qdrant_called = Cell::new(false);
    let injected_error =
        SystemdUnitError::MissingSection { unit: "repovecd.service", section: "Service" };

    let result = validate_daemon_startup_contracts_with(
        || Err(injected_error.clone()),
        || {
            qdrant_called.set(true);
            async { Ok(()) }
        },
    );

    assert!(
        matches!(result, Err(DaemonStartupError::SystemdUnit(error)) if error == injected_error)
    );
    assert!(!qdrant_called.get());
}

#[test]
fn startup_returns_exit_code_1_when_validation_fails() {
    let result = run_startup_with_systemd_error("repovecd.service");

    assert_eq!(result, Err(STARTUP_FAILURE_EXIT_CODE));
}

#[test]
fn startup_error_logging_preserves_systemd_unit_field() -> Result<(), String> {
    let (exit_code, logs) = repovec_test_helpers::capture_logs(|| {
        run_startup_with_systemd_error("repovec-mcpd.service")
    })?;

    repovec_test_helpers::ensure(
        exit_code == Err(STARTUP_FAILURE_EXIT_CODE),
        "startup error should map to exit code 1",
    )?;
    repovec_test_helpers::ensure_log_line_contains(
        &logs,
        "ERROR",
        "unit=repovec-mcpd.service",
        "startup failure log should preserve the systemd unit field",
    )
}

#[test]
fn qdrant_liveness_timeout_maps_to_startup_error() {
    let result = validate_qdrant_liveness_with_policy(
        || async { Err(QdrantLivenessError::Timeout { timeout: Duration::from_millis(5) }) },
        Duration::ZERO,
        Duration::from_millis(1),
    );

    assert!(matches!(
        result,
        Err(DaemonStartupError::QdrantLiveness(QdrantLivenessError::Timeout { .. }))
    ));
}

#[test]
fn qdrant_liveness_retries_transient_failures() {
    let attempts = Cell::new(0);

    let result = validate_qdrant_liveness_with_policy(
        || {
            let attempt = attempts.get();
            attempts.set(attempt + 1);
            std::future::ready(transient_qdrant_result(attempt))
        },
        Duration::from_millis(50),
        Duration::from_millis(1),
    );

    assert!(result.is_ok());
    assert_eq!(attempts.get(), 2);
}

#[rstest]
#[case::authentication_failed(QdrantLivenessError::AuthenticationFailed)]
#[case::invalid_endpoint(QdrantLivenessError::InvalidEndpoint {
    endpoint: String::from("not a uri"),
})]
#[case::missing_api_key_file(QdrantLivenessError::MissingApiKeyFile {
    path: "/tmp/missing-qdrant-api-key".into(),
})]
#[case::unreadable_api_key_file(QdrantLivenessError::UnreadableApiKeyFile {
    path: "/tmp/unreadable-qdrant-api-key".into(),
    source: std::io::Error::new(std::io::ErrorKind::PermissionDenied, "denied"),
})]
#[case::empty_api_key(QdrantLivenessError::EmptyApiKey)]
#[case::invalid_api_key(QdrantLivenessError::InvalidApiKey)]
#[case::missing_server_version(QdrantLivenessError::MissingServerVersion)]
fn qdrant_liveness_fails_permanent_errors_immediately(#[case] injected_error: QdrantLivenessError) {
    assert_permanent_qdrant_error_fails_immediately(injected_error);
}

fn run_startup_with_systemd_error(unit: &'static str) -> Result<(), i32> {
    let startup_error = DaemonStartupError::SystemdUnit(SystemdUnitError::MissingSection {
        unit,
        section: "Service",
    });
    Err(log_daemon_startup_error(startup_error))
}

fn assert_permanent_qdrant_error_fails_immediately(injected: QdrantLivenessError) {
    let attempts = Cell::new(0);
    let expected_error = injected.to_string();
    let injected_error = RefCell::new(Some(injected));

    let result = validate_qdrant_liveness_with_policy(
        || {
            attempts.set(attempts.get() + 1);
            let Some(failure) = injected_error.borrow_mut().take() else {
                panic!("permanent errors must not be retried");
            };
            std::future::ready(Err(failure))
        },
        Duration::from_millis(50),
        Duration::from_millis(1),
    );

    let Err(DaemonStartupError::QdrantLiveness(startup_error)) = result else {
        panic!("permanent Qdrant liveness errors should fail startup");
    };
    assert_eq!(startup_error.to_string(), expected_error);
    assert_eq!(attempts.get(), 1);
}

fn transient_qdrant_result(attempt: i32) -> Result<(), QdrantLivenessError> {
    match attempt {
        0 => Err(QdrantLivenessError::GrpcUnavailable {
            message: String::from("connection refused"),
        }),
        _ => Ok(()),
    }
}
