//! Process entry point for the repovec control-plane daemon.
//!
//! This binary initializes the process-wide `tracing` subscriber, runs the
//! shared systemd unit startup validation adapter from
//! [`repovec_core::appliance::systemd_units`], and treats any contract
//! violation as a fatal startup error. The substantive startup path lives in
//! [`startup`] so tests can verify the wiring without terminating the process.
//!
//! Unit tests delegate repeated daemon-startup assertions to
//! `repovec-test-helpers`, including log capture and snapshot coverage for the
//! tracing events emitted by the shared startup adapter.

fn main() {
    tracing_subscriber::fmt::init();
    if let Err(error) = startup() {
        std::process::exit(error);
    }
}

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
                write!(formatter, "failed to initialise async runtime: {error}")
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
fn startup() -> Result<(), i32> {
    validate_startup_contracts().map_err(|error| {
        tracing::error!(error = %error, "startup contract violation - aborting startup");
        1
    })
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
    H: FnOnce() -> F,
    F: Future<Output = Result<(), QdrantLivenessError>>,
{
    systemd_validator().map_err(StartupError::SystemdUnit)?;
    validate_qdrant_liveness_with(qdrant_liveness_check)
}

fn validate_qdrant_liveness_with<H, F>(health_check: H) -> Result<(), StartupError>
where
    H: FnOnce() -> F,
    F: Future<Output = Result<(), QdrantLivenessError>>,
{
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_io()
        .enable_time()
        .build()
        .map_err(StartupError::AsyncRuntime)?;
    let result = runtime.block_on(health_check());
    if result.is_ok() {
        tracing::debug!("Qdrant liveness validated");
    }
    result.map_err(StartupError::QdrantLiveness)
}
mod tests {
    //! Unit coverage for repovecd startup checks.

    use std::{cell::Cell, time::Duration};

    use repovec_core::appliance::{
        qdrant_liveness::QdrantLivenessError, systemd_units::SystemdUnitError,
    };

    use super::{StartupError, validate_qdrant_liveness_with, validate_startup_contracts_with};

    const UNIT: &str = "repovecd.service";

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
    fn startup_entrypoint_runs_real_checked_in_validation() -> Result<(), String> {
        repovec_test_helpers::assert_startup_entrypoint_runs_real_checked_in_validation(
            super::startup,
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
            SystemdUnitError::MissingSection { unit: "repovecd.service", section: "Service" };

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
        let result = validate_qdrant_liveness_with(|| async {
            Err(QdrantLivenessError::Timeout { timeout: Duration::from_millis(5) })
        });

        assert!(matches!(
            result,
            Err(StartupError::QdrantLiveness(QdrantLivenessError::Timeout { .. }))
        ));
    }
}
