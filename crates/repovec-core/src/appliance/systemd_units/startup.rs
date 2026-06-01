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
    fn run_startup_validation_returns_exit_code_1_when_validation_fails() {
        let result = run_startup_validation(|| {
            Err(SystemdUnitError::MissingSection { unit: "repovecd.service", section: "Service" })
        });

        assert_eq!(result, Err(1));
    }
}
