//! Process entry point for the repovec MCP bridge daemon.
//!
//! This binary initializes the process-wide `tracing` subscriber, validates the
//! checked-in systemd unit contract, validates Qdrant gRPC liveness, and treats
//! any startup contract violation as fatal. The substantive startup path lives
//! in [`startup`] so tests can verify the wiring without terminating the
//! process.
//!
//! Unit tests delegate repeated systemd-startup assertions to
//! `repovec-test-helpers`, including log capture and snapshot coverage for the
//! tracing events emitted by the shared startup adapter.

use std::{error::Error, fmt, future::Future, time::Duration};

use repovec_core::appliance::{
    qdrant_liveness::{
        QdrantLivenessConfig, QdrantLivenessError, QdrantStartupLivenessPolicy,
        check_qdrant_liveness, wait_for_qdrant_startup_liveness,
    },
    systemd_units::{SystemdUnitError, validate_and_trace_checked_in_units},
};

fn main() {
    tracing_subscriber::fmt::init();
    if let Err(error) = startup() {
        std::process::exit(error);
    }
}

#[derive(Debug)]
enum StartupError {
    SystemdUnit(SystemdUnitError),
    QdrantLiveness(QdrantLivenessError),
    AsyncRuntime(std::io::Error),
}

impl fmt::Display for StartupError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::SystemdUnit(error) => write!(formatter, "{error}"),
            Self::QdrantLiveness(error) => write!(formatter, "{error}"),
            Self::AsyncRuntime(error) => {
                write!(formatter, "failed to initialize async runtime: {error}")
            }
        }
    }
}

impl Error for StartupError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::SystemdUnit(error) => Some(error),
            Self::QdrantLiveness(error) => Some(error),
            Self::AsyncRuntime(error) => Some(error),
        }
    }
}

fn startup() -> Result<(), i32> { validate_startup_contracts().map_err(log_startup_error) }

const STARTUP_FAILURE_EXIT_CODE: i32 = 1;

fn log_startup_error(startup_error: StartupError) -> i32 {
    match startup_error {
        StartupError::SystemdUnit(error) => log_systemd_startup_error(&error),
        StartupError::QdrantLiveness(error) => log_qdrant_startup_error(&error),
        StartupError::AsyncRuntime(error) => log_async_runtime_startup_error(&error),
    }
    STARTUP_FAILURE_EXIT_CODE
}

fn log_systemd_startup_error(error: &SystemdUnitError) {
    tracing::error!(
        unit = %error.unit(),
        error = %error,
        "systemd unit contract violation - aborting startup",
    );
}

fn log_qdrant_startup_error(error: &QdrantLivenessError) {
    tracing::error!(
        error = %error,
        "Qdrant liveness validation failed - aborting startup",
    );
}

fn log_async_runtime_startup_error(error: &std::io::Error) {
    tracing::error!(
        error = %error,
        "async runtime initialization failed - aborting startup",
    );
}

fn validate_startup_contracts() -> Result<(), StartupError> {
    validate_startup_contracts_with(validate_and_trace_checked_in_units, || async {
        check_qdrant_liveness(&QdrantLivenessConfig::default()).await.map(|_report| ())
    })
}

fn validate_startup_contracts_with<S, H, F>(
    systemd_validator: S,
    qdrant_liveness_check: H,
) -> Result<(), StartupError>
where
    S: FnOnce() -> Result<(), SystemdUnitError>,
    H: FnMut() -> F,
    F: Future<Output = Result<(), QdrantLivenessError>>,
{
    systemd_validator().map_err(StartupError::SystemdUnit)?;
    tracing::debug!("systemd unit contract validated at daemon startup");
    validate_qdrant_liveness_with(qdrant_liveness_check)
}

const QDRANT_STARTUP_READINESS_TIMEOUT: Duration = Duration::from_secs(30);
const QDRANT_STARTUP_POLL_INTERVAL: Duration = Duration::from_millis(250);

fn validate_qdrant_liveness_with<H, F>(health_check: H) -> Result<(), StartupError>
where
    H: FnMut() -> F,
    F: Future<Output = Result<(), QdrantLivenessError>>,
{
    validate_qdrant_liveness_with_policy(
        health_check,
        QDRANT_STARTUP_READINESS_TIMEOUT,
        QDRANT_STARTUP_POLL_INTERVAL,
    )
}

fn validate_qdrant_liveness_with_policy<H, F>(
    health_check: H,
    readiness_timeout: Duration,
    poll_interval: Duration,
) -> Result<(), StartupError>
where
    H: FnMut() -> F,
    F: Future<Output = Result<(), QdrantLivenessError>>,
{
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_io()
        .enable_time()
        .build()
        .map_err(StartupError::AsyncRuntime)?;
    let policy = QdrantStartupLivenessPolicy::new(readiness_timeout, poll_interval);
    let result = runtime.block_on(wait_for_qdrant_startup_liveness(health_check, policy));
    if result.is_ok() {
        tracing::debug!("Qdrant liveness validated");
    }
    result.map_err(StartupError::QdrantLiveness)
}

#[cfg(test)]
mod tests {
    //! Unit coverage for repovec-mcpd startup checks.

    use std::{
        cell::{Cell, RefCell},
        time::Duration,
    };

    use repovec_core::appliance::{
        qdrant_liveness::QdrantLivenessError, systemd_units::SystemdUnitError,
    };

    use super::{
        StartupError, log_startup_error, validate_qdrant_liveness_with_policy,
        validate_startup_contracts_with,
    };

    const UNIT: &str = "repovec-mcpd.service";

    #[test]
    fn startup_succeeds_when_validation_passes() -> Result<(), String> {
        repovec_test_helpers::assert_startup_succeeds_when_validation_passes()
    }

    #[test]
    fn startup_logs_successful_validation() -> Result<(), String> {
        repovec_test_helpers::assert_startup_logs_successful_validation()
    }

    #[test]
    fn startup_success_log_matches_snapshot() -> Result<(), String> {
        repovec_test_helpers::assert_startup_success_log_snapshot()
    }

    #[test]
    fn startup_runs_real_checked_in_validation() -> Result<(), String> {
        repovec_test_helpers::assert_startup_runs_real_checked_in_validation()
    }

    #[test]
    fn validate_startup_contracts_runs_real_systemd_validation_and_qdrant() -> Result<(), String> {
        let (result, logs) = repovec_test_helpers::capture_logs(|| {
            validate_startup_contracts_with(
                repovec_core::appliance::systemd_units::validate_and_trace_checked_in_units,
                || async { Ok(()) },
            )
        })?;

        repovec_test_helpers::ensure(
            result.is_ok(),
            "checked-in units and injected Qdrant liveness should pass startup validation",
        )?;
        repovec_test_helpers::ensure_log_line_contains(
            &logs,
            "TRACE",
            "systemd unit contract validated",
            "startup contract validation should call the real systemd validator",
        )?;
        repovec_test_helpers::ensure_log_line_contains(
            &logs,
            "DEBUG",
            "systemd unit contract validated at daemon startup",
            "startup contract validation should log systemd success before Qdrant validation",
        )?;
        repovec_test_helpers::ensure_log_line_contains(
            &logs,
            "DEBUG",
            "Qdrant liveness validated",
            "startup contract validation should call the injected Qdrant check",
        )
    }

    #[test]
    fn startup_returns_exit_code_1_when_validation_fails() -> Result<(), String> {
        repovec_test_helpers::assert_startup_returns_exit_code_1_when_validation_fails(UNIT)
    }

    #[test]
    fn startup_logs_structured_validation_failure() -> Result<(), String> {
        repovec_test_helpers::assert_startup_logs_structured_validation_failure(UNIT)
    }

    #[test]
    fn startup_failure_log_matches_snapshot() -> Result<(), String> {
        repovec_test_helpers::assert_startup_failure_log_snapshot(UNIT)
    }

    #[test]
    fn validate_startup_contracts_with_succeeds_when_systemd_and_qdrant_pass() {
        validate_startup_contracts_with(|| Ok(()), || async { Ok(()) })
            .expect("startup contracts should pass");
    }

    #[test]
    fn validate_startup_contracts_with_skips_qdrant_after_systemd_failure() {
        let qdrant_called = Cell::new(false);
        let injected_error =
            SystemdUnitError::MissingSection { unit: "repovec-mcpd.service", section: "Service" };

        let result = validate_startup_contracts_with(
            || Err(injected_error.clone()),
            || {
                qdrant_called.set(true);
                async { Ok(()) }
            },
        );

        assert!(matches!(result, Err(StartupError::SystemdUnit(error)) if error == injected_error));
        assert!(!qdrant_called.get());
    }

    #[test]
    fn validate_qdrant_liveness_with_returns_injected_error() {
        let result = validate_qdrant_liveness_with_policy(
            || async { Err(QdrantLivenessError::Timeout { timeout: Duration::from_millis(5) }) },
            Duration::ZERO,
            Duration::from_millis(1),
        );

        assert!(matches!(
            result,
            Err(StartupError::QdrantLiveness(QdrantLivenessError::Timeout { .. }))
        ));
    }

    #[test]
    fn validate_qdrant_liveness_with_retries_transient_failures() {
        let attempts = Cell::new(0);

        let result = validate_qdrant_liveness_with_policy(
            || {
                let attempt = attempts.get();
                attempts.set(attempt + 1);
                async move { transient_qdrant_result(attempt) }
            },
            Duration::from_millis(50),
            Duration::from_millis(1),
        );

        assert!(result.is_ok());
        assert_eq!(attempts.get(), 2);
    }

    #[test]
    fn validate_qdrant_liveness_with_fails_permanent_errors_immediately() {
        for injected_error in permanent_qdrant_liveness_errors() {
            assert_permanent_qdrant_error_fails_immediately(injected_error);
        }
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

        let Err(StartupError::QdrantLiveness(startup_error)) = result else {
            panic!("permanent Qdrant liveness errors should fail startup");
        };
        assert_eq!(startup_error.to_string(), expected_error);
        assert_eq!(attempts.get(), 1);
    }

    fn permanent_qdrant_liveness_errors() -> Vec<QdrantLivenessError> {
        vec![
            QdrantLivenessError::AuthenticationFailed,
            QdrantLivenessError::InvalidEndpoint { endpoint: String::from("not a uri") },
            QdrantLivenessError::MissingApiKeyFile { path: "/tmp/missing-qdrant-api-key".into() },
            QdrantLivenessError::UnreadableApiKeyFile {
                path: "/tmp/unreadable-qdrant-api-key".into(),
                source: std::io::Error::new(std::io::ErrorKind::PermissionDenied, "denied"),
            },
            QdrantLivenessError::EmptyApiKey,
            QdrantLivenessError::InvalidApiKey,
        ]
    }

    fn transient_qdrant_result(attempt: i32) -> Result<(), QdrantLivenessError> {
        match attempt {
            0 => Err(QdrantLivenessError::GrpcUnavailable {
                message: String::from("connection refused"),
            }),
            _ => Ok(()),
        }
    }

    #[test]
    fn startup_error_logging_preserves_systemd_unit_field() -> Result<(), String> {
        let injected_error = StartupError::SystemdUnit(SystemdUnitError::MissingSection {
            unit: UNIT,
            section: "Service",
        });

        let (exit_code, logs) =
            repovec_test_helpers::capture_logs(|| log_startup_error(injected_error))?;

        repovec_test_helpers::ensure(exit_code == 1, "startup error should map to exit code 1")?;
        repovec_test_helpers::ensure_log_line_contains(
            &logs,
            "ERROR",
            &format!("unit={UNIT}"),
            "startup failure log should preserve the systemd unit field",
        )
    }
}
