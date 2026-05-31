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

    let _arguments = std::env::args_os();
    Ok(())
}

#[cfg(test)]
mod tests {
    //! Unit coverage for repovecd startup checks.

    use repovec_core::appliance::systemd_units::SystemdUnitError;

    use super::startup;

    #[test]
    fn startup_succeeds_when_validation_passes() {
        let result = startup(|| Ok(()));
        assert!(result.is_ok(), "startup should return Ok when validation passes");
    }

    #[test]
    fn startup_returns_exit_code_1_when_validation_fails() {
        let error =
            SystemdUnitError::MissingSection { unit: "repovecd.service", section: "Service" };
        let result = startup(|| Err(error));
        assert_eq!(result, Err(1), "startup should return Err(1) on validation failure");
    }
}
