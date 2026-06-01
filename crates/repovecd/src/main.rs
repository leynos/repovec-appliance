//! Process entry point for the repovec control-plane daemon.

fn main() {
    tracing_subscriber::fmt::init();
    if let Err(error) = repovec_core::appliance::systemd_units::run_startup_validation(
        repovec_core::appliance::systemd_units::validate_and_trace_checked_in_units,
    ) {
        std::process::exit(error);
    }
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
    fn startup_runs_real_checked_in_validation() -> Result<(), String> {
        repovec_test_helpers::assert_startup_runs_real_checked_in_validation()
    }

    #[test]
    fn startup_returns_exit_code_1_when_validation_fails() -> Result<(), String> {
        repovec_test_helpers::assert_startup_returns_exit_code_1_when_validation_fails(UNIT)
    }

    #[test]
    fn startup_logs_structured_validation_failure() -> Result<(), String> {
        repovec_test_helpers::assert_startup_logs_structured_validation_failure(UNIT)
    }
}
