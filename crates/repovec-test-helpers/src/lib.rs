//! Shared test utilities for repovec workspace crates.
//!
//! This crate provides a log-capture harness and assertion helpers for
//! testing daemon startup validation paths that use
//! [`repovec_core::appliance::systemd_units::run_startup_validation`].
//! It is consumed as a `[dev-dependencies]` entry in `repovecd` and
//! `repovec-mcpd`.
//!
//! ## Log capture
//!
//! [`startup_with_captured_logs`] installs a temporary `tracing` subscriber
//! that records all formatted log output into an in-memory buffer, then
//! invokes `run_startup_validation` with the supplied validator closure.
//! The captured text is returned alongside the validation result for
//! assertion.
//!
//! ## Assertion helpers
//!
//! The `assert_startup_*` functions are thin wrappers intended to be called
//! directly from test bodies in daemon crates. Each helper encapsulates one
//! observable startup behaviour (success, success logging, real-validator
//! invocation, exit-code mapping, structured failure diagnostics) and
//! returns `Result<(), String>` so the test body can use the `?` operator.

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
    ensure_log_line_contains(
        &logs,
        "DEBUG",
        "systemd unit contract validated at daemon startup",
        "startup should log validation success",
    )
}

/// Verifies daemon startup log output for a successful validation matches the
/// committed snapshot.
///
/// # Errors
///
/// Returns an error when startup validation fails or the captured log output
/// differs from the snapshot.
#[cfg(feature = "snapshots")]
pub fn assert_startup_success_log_snapshot() -> Result<(), String> {
    let (result, logs) = startup_with_captured_logs(validate_and_trace_checked_in_units)?;
    ensure(result.is_ok(), "startup should return Ok when validation passes")?;
    insta::assert_snapshot!("startup_success_log", logs);
    Ok(())
}

/// Verifies daemon startup log output for a validation failure matches the
/// committed snapshot.
///
/// # Errors
///
/// Returns an error when startup validation does not fail as expected or the
/// captured log output differs from the snapshot.
#[cfg(feature = "snapshots")]
pub fn assert_startup_failure_log_snapshot(unit: &'static str) -> Result<(), String> {
    let (result, logs) = startup_with_captured_logs(|| missing_section_error(unit))?;
    ensure(result == Err(1), "startup should return Err(1) on validation failure")?;
    insta::assert_snapshot!(format!("startup_failure_log_{}", unit.replace('.', "_")), logs);
    Ok(())
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
    ensure_log_line_contains(
        &logs,
        "TRACE",
        "systemd unit contract validated",
        "real validator should log successful validation",
    )?;
    ensure_log_line_contains(
        &logs,
        "DEBUG",
        "systemd unit contract validated at daemon startup",
        "startup should log successful validation at the daemon boundary",
    )
}

/// Verifies a daemon startup entrypoint is wired to the real checked-in validator.
///
/// # Errors
///
/// Returns an error when the startup entrypoint fails or omits the expected
/// real-validator trace and daemon-boundary debug events.
pub fn assert_startup_entrypoint_runs_real_checked_in_validation<F>(
    startup: F,
) -> Result<(), String>
where
    F: FnOnce() -> Result<(), i32>,
{
    let (result, logs) = capture_startup_logs(startup)?;

    ensure(result.is_ok(), "checked-in units should pass daemon startup validation")?;
    ensure_log_line_contains(
        &logs,
        "TRACE",
        "systemd unit contract validated",
        "daemon startup entrypoint should call the real checked-in validator",
    )?;
    ensure_log_line_contains(
        &logs,
        "DEBUG",
        "systemd unit contract validated at daemon startup",
        "daemon startup entrypoint should use the shared startup adapter",
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
    let unit_needle = format!("unit={unit}");
    ensure_log_line_contains(
        &logs,
        "ERROR",
        &unit_needle,
        "startup should log the failing unit as a structured field on the ERROR line",
    )?;
    let error_needle = format!("error={unit} is missing [Service]");
    ensure_log_line_contains(
        &logs,
        "ERROR",
        &error_needle,
        "startup should log the validation error as a structured field on the ERROR line",
    )?;
    ensure_log_line_contains(
        &logs,
        "ERROR",
        "systemd unit contract violation",
        "startup should log the fatal startup diagnostic",
    )
}

fn capture_startup_logs<F>(startup: F) -> Result<(Result<(), i32>, String), String>
where
    F: FnOnce() -> Result<(), i32>,
{
    let logs = CapturedLogs::default();
    let subscriber = tracing_subscriber::fmt()
        .with_ansi(false)
        .without_time()
        .with_target(false)
        .with_max_level(tracing::Level::TRACE)
        .with_writer(logs.clone())
        .finish();
    let result = tracing::subscriber::with_default(subscriber, startup);
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

const fn missing_section_error(unit: &'static str) -> Result<(), SystemdUnitError> {
    Err(SystemdUnitError::MissingSection { unit, section: "Service" })
}

fn ensure(condition: bool, message: &str) -> Result<(), String> {
    if condition { Ok(()) } else { Err(message.to_owned()) }
}

fn ensure_log_line_contains(
    logs: &str,
    level: &str,
    needle: &str,
    message: &str,
) -> Result<(), String> {
    let found = logs.lines().any(|line| line.contains(level) && line.contains(needle));
    ensure(found, &format!("{message}: {logs}"))
}
