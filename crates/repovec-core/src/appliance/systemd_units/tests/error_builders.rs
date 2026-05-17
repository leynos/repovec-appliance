//! Typed error constructors for systemd unit validator tests.

use super::SystemdUnitError;

pub(super) fn missing(
    unit: &'static str,
    key: &'static str,
    dependency: &'static str,
) -> SystemdUnitError {
    SystemdUnitError::MissingDependency { unit, section: "Unit", key, dependency }
}

pub(super) fn missing_install(unit: &'static str, dependency: &'static str) -> SystemdUnitError {
    SystemdUnitError::MissingDependency { unit, section: "Install", key: "WantedBy", dependency }
}

pub(super) fn quadlet_source(
    unit: &'static str,
    key: &'static str,
    dependency: &str,
) -> SystemdUnitError {
    SystemdUnitError::UsesQuadletSourceDependency {
        unit,
        section: "Unit",
        key,
        dependency: dependency.to_owned(),
    }
}

pub(super) fn exec_start(
    unit: &'static str,
    expected: &'static str,
    actual: &str,
) -> SystemdUnitError {
    SystemdUnitError::IncorrectExecStart { unit, expected, actual: actual.to_owned() }
}

pub(super) fn service_directive(
    unit: &'static str,
    key: &'static str,
    expected: &'static str,
    actual: &str,
) -> SystemdUnitError {
    SystemdUnitError::IncorrectServiceDirective { unit, key, expected, actual: actual.to_owned() }
}
