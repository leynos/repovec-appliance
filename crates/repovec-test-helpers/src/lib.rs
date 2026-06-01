//! Shared test utilities for repovec workspace crates.

use std::{
    io::{self, Write},
    sync::{Arc, Mutex},
};

use repovec_core::appliance::systemd_units::{
    SystemdUnitError, run_startup_validation, validate_and_trace_checked_in_units,
};
use tracing_subscriber::fmt::MakeWriter;

/// Captures daemon startup validation logs for assertions.
///
/// Runs [`run_startup_validation`] with a temporary tracing subscriber that
/// records formatted logs into memory.
///
/// # Errors
///
/// Returns an error string if the captured log buffer cannot be read as UTF-8.
pub fn startup_with_captured_logs<F>(validator: F) -> Result<(Result<(), i32>, String), String>
where
    F: FnOnce() -> Result<(), SystemdUnitError>,
{
    let logs = CapturedLogs::default();
    let subscriber = tracing_subscriber::fmt()
        .with_ansi(false)
        .without_time()
        .with_target(false)
        .with_max_level(tracing::Level::TRACE)
        .with_writer(logs.clone())
        .finish();
    let result =
        tracing::subscriber::with_default(subscriber, || run_startup_validation(validator));
    Ok((result, logs.content()?))
}

/// Verifies that startup validation succeeds for an injected passing validator.
///
/// # Errors
///
/// Returns an error when startup validation unexpectedly fails.
pub fn assert_startup_succeeds_when_validation_passes() -> Result<(), String> {
    let result = run_startup_validation(|| Ok(()));
    ensure(result.is_ok(), "startup should return Ok when validation passes")
}

/// Verifies that successful startup validation emits an operator-visible log.
///
/// # Errors
///
/// Returns an error when startup validation fails or the expected log entry is
/// missing.
pub fn assert_startup_logs_successful_validation() -> Result<(), String> {
    let (result, logs) = startup_with_captured_logs(|| Ok(()))?;

    ensure(result.is_ok(), "startup should return Ok when validation passes")?;
    ensure_log_contains(&logs, "DEBUG", "startup should log successful validation")?;
    ensure_log_contains(
        &logs,
        "systemd unit contract validated at daemon startup",
        "startup should log validation success",
    )
}

/// Verifies daemon startup invokes the real checked-in unit validator.
///
/// # Errors
///
/// Returns an error when the real validator fails or its trace/debug logs are
/// missing.
pub fn assert_startup_runs_real_checked_in_validation() -> Result<(), String> {
    let (result, logs) = startup_with_captured_logs(validate_and_trace_checked_in_units)?;

    ensure(result.is_ok(), "checked-in units should pass daemon startup validation")?;
    ensure_log_contains(&logs, "TRACE", "real validator should log its successful trace event")?;
    ensure_log_contains(
        &logs,
        "systemd unit contract validated",
        "real validator should log successful validation",
    )?;
    ensure_log_contains(
        &logs,
        "DEBUG",
        "startup should log successful validation at the daemon boundary",
    )
}

/// Verifies startup validation maps validation failures to exit code 1.
///
/// # Errors
///
/// Returns an error when startup validation does not return `Err(1)`.
pub fn assert_startup_returns_exit_code_1_when_validation_fails(
    unit: &'static str,
) -> Result<(), String> {
    let result = run_startup_validation(|| missing_section_error(unit));
    ensure(result == Err(1), "startup should return Err(1) on validation failure")
}

/// Verifies startup validation emits structured failure diagnostics.
///
/// # Errors
///
/// Returns an error when startup validation does not fail as expected or the
/// expected structured log fields are missing.
pub fn assert_startup_logs_structured_validation_failure(unit: &'static str) -> Result<(), String> {
    let (result, logs) = startup_with_captured_logs(|| missing_section_error(unit))?;

    ensure(result == Err(1), "startup should return Err(1) on validation failure")?;
    ensure_log_contains(&logs, "ERROR", "startup should log validation failures")?;
    ensure_log_contains(
        &logs,
        &format!("unit={unit}"),
        "startup should log the failing unit as a structured field",
    )?;
    ensure_log_contains(
        &logs,
        &format!("error={unit} is missing [Service]"),
        "startup should log the validation error as a structured field",
    )?;
    ensure_log_contains(
        &logs,
        "systemd unit contract violation",
        "startup should log the fatal startup diagnostic",
    )
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
        String::from_utf8(buffer.clone()).map_err(|error| error.to_string())
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

const fn missing_section_error(unit: &'static str) -> Result<(), SystemdUnitError> {
    Err(SystemdUnitError::MissingSection { unit, section: "Service" })
}

fn ensure(condition: bool, message: &str) -> Result<(), String> {
    if condition { Ok(()) } else { Err(message.to_owned()) }
}

fn ensure_log_contains(logs: &str, needle: &str, message: &str) -> Result<(), String> {
    ensure(logs.contains(needle), &format!("{message}: {logs}"))
}
