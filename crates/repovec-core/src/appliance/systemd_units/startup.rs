//! Daemon startup validation helpers for checked-in systemd units.

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

    use super::{
        SystemdUnitError, validate_and_trace_checked_in_units,
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
}
