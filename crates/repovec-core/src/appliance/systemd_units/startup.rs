//! Daemon startup validation helpers for checked-in systemd units.
//!
//! This module is a private submodule of
//! [`crate::appliance::systemd_units`]. Its public items are re-exported
//! from the parent module, so callers should import them from
//! [`crate::appliance::systemd_units`] rather than from this path directly.
//!
//! ## Entry points
//!
//! - [`validate_and_trace_checked_in_units`] — validates the three
//!   embedded systemd unit assets and emits a `tracing::trace!` event on
//!   success. Daemon binaries call this as the first substantive action in
//!   `main()`.
//! - [`run_startup_validation`] — runs an injected validator closure,
//!   emits structured `tracing::error!` diagnostics on failure, and maps
//!   any [`SystemdUnitError`] to `Err(1)` so the caller can exit with the
//!   returned code.
//!
//! The private helper `validate_and_trace_systemd_units_with` is an
//! injection seam used by tests and by `validate_and_trace_checked_in_units`
//! itself.

use super::{SystemdUnitError, validate_checked_in_systemd_units};

/// Validates the checked-in systemd units and traces successful validation.
///
/// # Errors
///
/// Returns [`SystemdUnitError`] when a checked-in unit no longer satisfies the
/// appliance service-layout contract.
///
/// # Examples
///
/// ```
/// use repovec_core::appliance::systemd_units::validate_and_trace_checked_in_units;
///
/// validate_and_trace_checked_in_units().expect("the checked-in units remain valid");
/// ```
pub fn validate_and_trace_checked_in_units() -> Result<(), SystemdUnitError> {
    validate_and_trace_systemd_units_with(validate_checked_in_systemd_units)
}

/// Runs daemon startup validation and emits boundary diagnostics.
///
/// Accepts a `validator` closure so daemon binaries can use the checked-in unit
/// validator while tests can inject deterministic success or failure cases.
///
/// Returns `Ok(())` when validation passes or `Err(1)` when validation fails
/// and the process should terminate with a non-zero status.
///
/// # Errors
///
/// Returns `Err(1)` when `validator` returns a [`SystemdUnitError`]. The error
/// is logged with structured `unit` and `error` fields before returning.
///
/// # Examples
///
/// ```
/// use repovec_core::appliance::systemd_units::{
///     run_startup_validation, validate_and_trace_checked_in_units,
/// };
///
/// run_startup_validation(validate_and_trace_checked_in_units)
///     .expect("checked-in units satisfy the startup contract");
/// ```
pub fn run_startup_validation<F>(validator: F) -> Result<(), i32>
where
    F: FnOnce() -> Result<(), SystemdUnitError>,
{
    validator().map_err(|error| {
        tracing::error!(
            unit = %error.unit(),
            error = %error,
            "systemd unit contract violation — aborting startup",
        );
        1
    })?;

    tracing::debug!("systemd unit contract validated at daemon startup");
    Ok(())
}

fn validate_and_trace_systemd_units_with<F>(validator: F) -> Result<(), SystemdUnitError>
where
    F: FnOnce() -> Result<(), SystemdUnitError>,
{
    let result = validator();
    if result.is_ok() {
        tracing::trace!("systemd unit contract validated");
    }
    result
}

#[cfg(test)]
mod tests {
    //! Unit coverage for daemon startup systemd validation helpers.

    use std::{
        io::{self, Write},
        sync::{Arc, Mutex},
    };

    use tracing_subscriber::fmt::MakeWriter;

    use super::{
        SystemdUnitError, run_startup_validation, validate_and_trace_checked_in_units,
        validate_and_trace_systemd_units_with,
    };

    #[test]
    fn validate_and_trace_checked_in_units_succeeds_for_checked_in_units() {
        validate_and_trace_checked_in_units()
            .expect("checked-in units must satisfy the contract at compile time");
    }

    #[test]
    fn validate_and_trace_systemd_units_with_returns_injected_error() {
        let injected_error =
            SystemdUnitError::MissingSection { unit: "repovecd.service", section: "Service" };

        let result = validate_and_trace_systemd_units_with(|| {
            Err(SystemdUnitError::MissingSection { unit: "repovecd.service", section: "Service" })
        });

        assert_eq!(result, Err(injected_error));
    }

    #[test]
    fn validate_and_trace_systemd_units_with_traces_success() -> Result<(), String> {
        let (result, logs) = capture_logs(|| validate_and_trace_systemd_units_with(|| Ok(())))?;

        ensure(result == Ok(()), "validation should succeed")?;
        assert_log_line_contains(
            &logs,
            "TRACE",
            "systemd unit contract validated",
            "successful validation should emit the trace event",
        )
    }

    #[test]
    fn run_startup_validation_returns_exit_code_1_when_validation_fails() {
        let result = run_startup_validation(|| {
            Err(SystemdUnitError::MissingSection { unit: "repovecd.service", section: "Service" })
        });

        assert_eq!(result, Err(1));
    }

    #[test]
    fn run_startup_validation_logs_structured_error_when_validation_fails() -> Result<(), String> {
        let (result, logs) = capture_logs(|| {
            run_startup_validation(|| {
                Err(SystemdUnitError::MissingSection {
                    unit: "repovecd.service",
                    section: "Service",
                })
            })
        })?;

        ensure(result == Err(1), "startup validation should return exit code 1")?;
        assert_log_line_contains(
            &logs,
            "ERROR",
            "unit=repovecd.service",
            "startup failure should log the failing unit",
        )?;
        assert_log_line_contains(
            &logs,
            "ERROR",
            "error=repovecd.service is missing [Service]",
            "startup failure should log the validation error",
        )?;
        assert_log_line_contains(
            &logs,
            "ERROR",
            "systemd unit contract violation",
            "startup failure should log the fatal diagnostic",
        )
    }

    fn capture_logs<T, F>(action: F) -> Result<(T, String), String>
    where
        F: FnOnce() -> T,
    {
        let logs = CapturedLogs::default();
        let subscriber = tracing_subscriber::fmt()
            .with_ansi(false)
            .without_time()
            .with_target(false)
            .with_max_level(tracing::Level::TRACE)
            .with_writer(logs.clone())
            .finish();
        let result = tracing::subscriber::with_default(subscriber, action);
        Ok((result, logs.content()?))
    }

    #[derive(Clone, Default)]
    struct CapturedLogs {
        buffer: Arc<Mutex<Vec<u8>>>,
    }

    impl CapturedLogs {
        fn content(&self) -> Result<String, String> {
            let buffer = self
                .buffer
                .lock()
                .map_err(|_| "captured log buffer should not be poisoned".to_owned())?;
            std::str::from_utf8(&buffer).map(ToOwned::to_owned).map_err(|error| error.to_string())
        }
    }

    struct CapturedLogWriter {
        buffer: Arc<Mutex<Vec<u8>>>,
    }

    impl Write for CapturedLogWriter {
        fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
            let mut buffer =
                self.buffer.lock().map_err(|_| io::Error::other("captured log buffer poisoned"))?;
            buffer.extend_from_slice(bytes);
            Ok(bytes.len())
        }

        fn flush(&mut self) -> io::Result<()> { Ok(()) }
    }

    impl<'writer> MakeWriter<'writer> for CapturedLogs {
        type Writer = CapturedLogWriter;

        fn make_writer(&'writer self) -> Self::Writer {
            CapturedLogWriter { buffer: Arc::clone(&self.buffer) }
        }
    }

    fn assert_log_line_contains(
        logs: &str,
        level: &str,
        needle: &str,
        message: &str,
    ) -> Result<(), String> {
        let found = logs.lines().any(|line| line.contains(level) && line.contains(needle));
        if found { Ok(()) } else { Err(format!("{message}: {logs}")) }
    }

    fn ensure(condition: bool, message: &str) -> Result<(), String> {
        if condition { Ok(()) } else { Err(message.to_owned()) }
    }
}
