//! Shared startup orchestration for repovec daemon binaries.

use std::{error::Error, fmt, future::Future, time::Duration};

use super::{
    qdrant_liveness::{
        QdrantLivenessConfig, QdrantLivenessError, QdrantStartupLivenessPolicy,
        check_qdrant_liveness, wait_for_qdrant_startup_liveness,
    },
    systemd_units::{SystemdUnitError, validate_and_trace_checked_in_units},
};

/// Process status returned when any required daemon startup check fails.
const STARTUP_FAILURE_EXIT_CODE: i32 = 1;
/// Maximum startup retry window before an unavailable Qdrant aborts the daemon.
const QDRANT_STARTUP_READINESS_TIMEOUT: Duration = Duration::from_secs(30);
/// Delay between retryable Qdrant probes during the startup window.
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

/// Runs an injected health check with the appliance startup retry timings.
///
/// For example, a check that becomes healthy on retry allows startup to proceed;
/// an authentication failure is returned immediately as a startup error.
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

/// Runs an injected health check on a temporary runtime using explicit timings.
///
/// For example, a short readiness window bounds retries in a startup test;
/// exhausting it returns a liveness error rather than starting the daemon.
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

/// Emits the category-specific startup failure and returns the failure status.
///
/// For example, a systemd contract error is logged with its unit and produces
/// exit status 1 for the daemon entrypoint.
fn log_daemon_startup_error(startup_error: DaemonStartupError) -> i32 {
    match startup_error {
        DaemonStartupError::SystemdUnit(error) => log_systemd_startup_error(&error),
        DaemonStartupError::QdrantLiveness(error) => log_qdrant_startup_error(&error),
        DaemonStartupError::AsyncRuntime(error) => log_async_runtime_startup_error(&error),
    }
    STARTUP_FAILURE_EXIT_CODE
}

/// Records a rejected systemd unit with the unit name and underlying error.
///
/// For example, a missing ordering directive emits an error identifying the
/// affected unit so an operator can repair the startup contract.
fn log_systemd_startup_error(error: &SystemdUnitError) {
    tracing::error!(
        unit = %error.unit(),
        error = %error,
        "systemd unit contract violation - aborting startup",
    );
}

/// Records a failed Qdrant startup probe with its stable error category.
///
/// For example, an expired readiness window emits a timeout category alongside
/// the error details for startup diagnosis.
fn log_qdrant_startup_error(error: &QdrantLivenessError) {
    tracing::error!(
        error = %error,
        error_category = super::qdrant_liveness::qdrant_liveness_error_category(error),
        "Qdrant liveness validation failed - aborting startup",
    );
}

/// Records an I/O error preventing construction of the startup-only runtime.
///
/// For example, an unavailable runtime resource emits its error before the
/// entrypoint returns a failure status.
fn log_async_runtime_startup_error(error: &std::io::Error) {
    tracing::error!(
        error = %error,
        "async runtime initialization failed - aborting startup",
    );
}

#[cfg(test)]
#[path = "daemon_startup_tests.rs"]
mod tests;
