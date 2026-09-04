//! Shared startup orchestration for repovec daemon binaries.

use std::{error::Error, fmt, future::Future, time::Duration};

use super::{
    directory_layout::live::{LiveLayoutError, verify_live_layout},
    qdrant_liveness::{
        QdrantLivenessConfig, QdrantLivenessError, QdrantStartupLivenessPolicy,
        check_qdrant_liveness, wait_for_qdrant_startup_liveness,
    },
    systemd_units::{SystemdUnitError, validate_and_trace_checked_in_units},
};
use crate::RuntimePaths;

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
    /// The live on-disk directory layout is missing or misprovisioned.
    LiveLayout(LiveLayoutError),
    /// The startup-only async runtime could not be initialized.
    AsyncRuntime(std::io::Error),
}

impl fmt::Display for DaemonStartupError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::SystemdUnit(error) => write!(formatter, "{error}"),
            Self::QdrantLiveness(error) => write!(formatter, "{error}"),
            Self::LiveLayout(error) => write!(formatter, "{error}"),
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
            Self::LiveLayout(error) => Some(error),
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
    validate_daemon_startup_contracts_with_live(
        validate_and_trace_checked_in_units,
        || verify_live_layout(&RuntimePaths::appliance_defaults()),
        || async {
            let config = QdrantLivenessConfig::for_appliance()?;
            check_qdrant_liveness(&config).await.map(|_report| ())
        },
    )
}

/// Validates daemon startup contracts including the live directory pre-flight.
///
/// This is the shared test seam for daemon binaries when callers want to inject
/// the live directory-layout check. Production callers should use
/// [`validate_daemon_startup_contracts`].
///
/// # Errors
///
/// Returns [`DaemonStartupError`] when the injected systemd validator, live
/// directory-layout check, Qdrant health check, or runtime construction fails.
pub fn validate_daemon_startup_contracts_with_live<S, L, H, F>(
    systemd_validator: S,
    live_layout_check: L,
    qdrant_liveness_check: H,
) -> Result<(), DaemonStartupError>
where
    S: FnOnce() -> Result<(), SystemdUnitError>,
    L: FnOnce() -> Result<(), LiveLayoutError>,
    H: FnMut() -> F,
    F: Future<Output = Result<(), QdrantLivenessError>>,
{
    validate_systemd_and_live(systemd_validator, live_layout_check)?;
    validate_qdrant_liveness_with(qdrant_liveness_check)
}

fn validate_systemd_and_live<S, L>(
    systemd_validator: S,
    live_layout_check: L,
) -> Result<(), DaemonStartupError>
where
    S: FnOnce() -> Result<(), SystemdUnitError>,
    L: FnOnce() -> Result<(), LiveLayoutError>,
{
    check_systemd_units(systemd_validator)?;
    check_live_layout(live_layout_check)?;
    Ok(())
}

fn check_systemd_units<S>(systemd_validator: S) -> Result<(), DaemonStartupError>
where
    S: FnOnce() -> Result<(), SystemdUnitError>,
{
    systemd_validator().map_err(DaemonStartupError::SystemdUnit)?;
    tracing::debug!("systemd unit contract validated at daemon startup");
    Ok(())
}

fn check_live_layout<L>(live_layout_check: L) -> Result<(), DaemonStartupError>
where
    L: FnOnce() -> Result<(), LiveLayoutError>,
{
    live_layout_check().map_err(DaemonStartupError::LiveLayout)?;
    tracing::debug!("live directory layout pre-flight at daemon startup");
    Ok(())
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
        DaemonStartupError::LiveLayout(error) => log_live_layout_startup_error(&error),
        DaemonStartupError::AsyncRuntime(error) => log_async_runtime_startup_error(&error),
    }
    STARTUP_FAILURE_EXIT_CODE
}

fn log_live_layout_startup_error(error: &LiveLayoutError) {
    tracing::error!(
        path = %error.path().unwrap_or("(unknown)"),
        error = %error,
        "live directory layout violation - aborting startup",
    );
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
mod tests;
