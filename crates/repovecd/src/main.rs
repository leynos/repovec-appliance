//! Process entry point for the repovec control-plane daemon.

fn main() {
    tracing_subscriber::fmt::init();
    if let Err(error) =
        startup(repovec_core::appliance::systemd_units::validate_and_trace_checked_in_units)
    {
        std::process::exit(error);
    }
}

/// Run daemon startup checks.
///
/// Accepts a `validator` closure so the validation step can be replaced in
/// tests without spawning a real process.
///
/// Returns `Ok(())` when all checks pass or `Err(exit_code)` when a check
/// fails and the process should terminate with that code.
fn startup<F>(validator: F) -> Result<(), i32>
where
    F: FnOnce() -> Result<(), repovec_core::appliance::systemd_units::SystemdUnitError>,
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

#[cfg(test)]
mod tests {
    //! Unit coverage for repovecd startup checks.

    use std::{
        io::{self, Write},
        sync::{Arc, Mutex},
    };

    use repovec_core::appliance::systemd_units::{
        SystemdUnitError, validate_and_trace_checked_in_units,
    };
    use tracing_subscriber::fmt::MakeWriter;

    use super::startup;

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

    fn startup_with_captured_logs<F>(validator: F) -> Result<(Result<(), i32>, String), String>
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
        let result = tracing::subscriber::with_default(subscriber, || startup(validator));
        Ok((result, logs.content()?))
    }

    #[test]
    fn startup_succeeds_when_validation_passes() {
        let result = startup(|| Ok(()));
        assert!(result.is_ok(), "startup should return Ok when validation passes");
    }

    #[test]
    fn startup_logs_successful_validation() {
        let (result, logs) =
            startup_with_captured_logs(|| Ok(())).expect("captured logs should be readable");

        assert!(result.is_ok(), "startup should return Ok when validation passes");
        assert!(logs.contains("DEBUG"), "startup should log successful validation: {logs}");
        assert!(
            logs.contains("systemd unit contract validated at daemon startup"),
            "startup should log validation success: {logs}"
        );
    }

    #[test]
    fn startup_runs_real_checked_in_validation() {
        let (result, logs) = startup_with_captured_logs(validate_and_trace_checked_in_units)
            .expect("captured logs should be readable");

        assert!(result.is_ok(), "checked-in units should pass daemon startup validation");
        assert!(
            logs.contains("TRACE"),
            "real validator should log its successful trace event: {logs}"
        );
        assert!(
            logs.contains("systemd unit contract validated"),
            "real validator should log successful validation: {logs}"
        );
        assert!(
            logs.contains("DEBUG"),
            "startup should log successful validation at the daemon boundary: {logs}"
        );
    }

    #[test]
    fn startup_returns_exit_code_1_when_validation_fails() {
        let error =
            SystemdUnitError::MissingSection { unit: "repovecd.service", section: "Service" };
        let result = startup(|| Err(error));
        assert_eq!(result, Err(1), "startup should return Err(1) on validation failure");
    }

    #[test]
    fn startup_logs_structured_validation_failure() {
        let error =
            SystemdUnitError::MissingSection { unit: "repovecd.service", section: "Service" };

        let (result, logs) =
            startup_with_captured_logs(|| Err(error)).expect("captured logs should be readable");

        assert_eq!(result, Err(1), "startup should return Err(1) on validation failure");
        assert!(logs.contains("ERROR"), "startup should log validation failures: {logs}");
        assert!(
            logs.contains("unit=repovecd.service"),
            "startup should log the failing unit as a structured field: {logs}"
        );
        assert!(
            logs.contains("error=repovecd.service is missing [Service]"),
            "startup should log the validation error as a structured field: {logs}"
        );
        assert!(
            logs.contains("systemd unit contract violation"),
            "startup should log the fatal startup diagnostic: {logs}"
        );
    }
}
