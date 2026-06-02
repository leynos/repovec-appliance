//! Process entry point for the repovec control-plane daemon.
//!
//! This binary initialises the process-wide `tracing` subscriber, runs the
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

fn startup() -> Result<(), i32> {
    repovec_core::appliance::systemd_units::run_startup_validation(
        repovec_core::appliance::systemd_units::validate_and_trace_checked_in_units,
    )
}

#[cfg(test)]
mod tests {
    //! Unit coverage for repovecd startup checks.

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
}
