//! Shared startup orchestration for repovec daemon binaries.

use std::{error::Error, fmt, future::Future, time::Duration};

use super::{
    qdrant_liveness::{
        QdrantLivenessConfig, QdrantLivenessError, QdrantStartupLivenessPolicy,
        check_qdrant_liveness, wait_for_qdrant_startup_liveness,
    },
    systemd_units::{SystemdUnitError, validate_and_trace_checked_in_units},
};

const STARTUP_FAILURE_EXIT_CODE: i32 = 1;
const QDRANT_STARTUP_READINESS_TIMEOUT: Duration = Duration::from_secs(30);
const QDRANT_STARTUP_POLL_INTERVAL: Duration = Duration::from_millis(250);

/// Startup validation failures that abort daemon startup.
#[derive(Debug)]
pub enum DaemonStartupError {
    /// A checked-in systemd unit violates the appliance startup contract.
    SystemdUnit(SystemdUnitError),
    /// Qdrant liveness could not be established.
    QdrantLiveness(QdrantLivenessError),
    /// The startup-only async runtime could not be initialized.
    AsyncRuntime(std::io::Error),
}

impl fmt::Display for DaemonStartupError {
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

impl Error for DaemonStartupError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::SystemdUnit(error) => Some(error),
            Self::QdrantLiveness(error) => Some(error),
            Self::AsyncRuntime(error) => Some(error),
        }
    }
}

/// Runs the shared daemon startup contract and maps failures to exit codes.
///
/// # Examples
///
/// ```no_run
/// use repovec_core::appliance::daemon_startup::run_daemon_startup;
///
/// assert_eq!(run_daemon_startup(), Ok(()));
/// ```
///
/// # Errors
///
/// Returns the daemon startup failure exit code when a startup contract fails.
pub fn run_daemon_startup() -> Result<(), i32> {
    validate_daemon_startup_contracts().map_err(log_daemon_startup_error)
}

/// Validates checked-in systemd units and default Qdrant liveness.
///
/// # Examples
///
/// ```no_run
/// use repovec_core::appliance::daemon_startup::validate_daemon_startup_contracts;
///
/// assert!(validate_daemon_startup_contracts().is_ok());
/// ```
///
/// # Errors
///
/// Returns [`DaemonStartupError`] when either startup contract fails or the
/// startup-only async runtime cannot be built.
pub fn validate_daemon_startup_contracts() -> Result<(), DaemonStartupError> {
    validate_daemon_startup_contracts_with(validate_and_trace_checked_in_units, || async {
        let config = QdrantLivenessConfig::for_appliance()?;
        check_qdrant_liveness(&config).await.map(|_report| ())
    })
}

/// Validates daemon startup contracts using injected boundaries.
///
/// This is the shared test seam for daemon binaries. Production callers should
/// use [`validate_daemon_startup_contracts`].
///
/// # Examples
///
/// ```no_run
/// use repovec_core::appliance::{
///     daemon_startup::validate_daemon_startup_contracts_with,
///     qdrant_liveness::QdrantLivenessError,
///     systemd_units::SystemdUnitError,
/// };
///
/// let result = validate_daemon_startup_contracts_with(
///     || Ok::<(), SystemdUnitError>(()),
///     || async { Ok::<(), QdrantLivenessError>(()) },
/// );
/// assert!(result.is_ok());
/// ```
///
/// # Errors
///
/// Returns [`DaemonStartupError`] when the injected systemd validator, Qdrant
/// health check, or runtime construction fails.
pub fn validate_daemon_startup_contracts_with<S, H, F>(
    systemd_validator: S,
    qdrant_liveness_check: H,
) -> Result<(), DaemonStartupError>
where
    S: FnOnce() -> Result<(), SystemdUnitError>,
    H: FnMut() -> F,
    F: Future<Output = Result<(), QdrantLivenessError>>,
{
    systemd_validator().map_err(DaemonStartupError::SystemdUnit)?;
    tracing::debug!("systemd unit contract validated at daemon startup");
    validate_qdrant_liveness_with(qdrant_liveness_check)
}

fn validate_qdrant_liveness_with<H, F>(health_check: H) -> Result<(), DaemonStartupError>
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
) -> Result<(), DaemonStartupError>
where
    H: FnMut() -> F,
    F: Future<Output = Result<(), QdrantLivenessError>>,
{
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_io()
        .enable_time()
        .build()
        .map_err(DaemonStartupError::AsyncRuntime)?;
    let policy = QdrantStartupLivenessPolicy::new(readiness_timeout, poll_interval);
    let result = runtime.block_on(wait_for_qdrant_startup_liveness(health_check, policy));
    if result.is_ok() {
        tracing::debug!("Qdrant liveness validated");
    }
    result.map_err(DaemonStartupError::QdrantLiveness)
}

fn log_daemon_startup_error(startup_error: DaemonStartupError) -> i32 {
    match startup_error {
        DaemonStartupError::SystemdUnit(error) => log_systemd_startup_error(&error),
        DaemonStartupError::QdrantLiveness(error) => log_qdrant_startup_error(&error),
        DaemonStartupError::AsyncRuntime(error) => log_async_runtime_startup_error(&error),
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
        error_category = super::qdrant_liveness::qdrant_liveness_error_category(error),
        "Qdrant liveness validation failed - aborting startup",
    );
}

fn log_async_runtime_startup_error(error: &std::io::Error) {
    tracing::error!(
        error = %error,
        "async runtime initialization failed - aborting startup",
    );
}

#[cfg(test)]
mod tests {
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
    fn qdrant_liveness_fails_permanent_errors_immediately(
        #[case] injected_error: QdrantLivenessError,
    ) {
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
}
