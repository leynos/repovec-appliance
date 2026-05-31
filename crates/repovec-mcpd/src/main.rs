//! Process entry point for the repovec MCP bridge daemon.

fn main() {
    tracing_subscriber::fmt::init();
    if let Err(error) =
        startup(repovec_core::appliance::systemd_units::validate_and_trace_checked_in_units)
    {
        std::process::exit(error);
    }
}

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
    //! Unit coverage for repovec-mcpd startup checks.

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
            SystemdUnitError::MissingSection { unit: "repovec-mcpd.service", section: "Service" };
        let result = startup(|| Err(error));
        assert_eq!(result, Err(1), "startup should return Err(1) on validation failure");
    }
}
