//! Validation helpers for checked-in repovec systemd service units.

mod error;
mod parser;

pub use error::SystemdUnitError;
use parser::ParsedUnit;

const CHECKED_IN_REPOVEC_TARGET: &str =
    include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/../../packaging/systemd/repovec.target"));
const CHECKED_IN_REPOVECD_SERVICE: &str =
    include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/../../packaging/systemd/repovecd.service"));
const CHECKED_IN_REPOVEC_MCPD_SERVICE: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../packaging/systemd/repovec-mcpd.service"
));

const UNIT_SECTION: &str = "Unit";
const SERVICE_SECTION: &str = "Service";
const INSTALL_SECTION: &str = "Install";
const QDRANT_SERVICE: &str = "qdrant.service";
const REPOVECD_SERVICE: &str = "repovecd.service";
const REPOVEC_MCPD_SERVICE: &str = "repovec-mcpd.service";
const CLOUDFLARED_SERVICE: &str = "cloudflared.service";
const MULTI_USER_TARGET: &str = "multi-user.target";
const SERVICE_USER: &str = "repovec";
const SERVICE_GROUP: &str = "repovec";
const WORKING_DIRECTORY: &str = "/var/lib/repovec";
const HOME_ENVIRONMENT: &str = "HOME=/var/lib/repovec";
const REPOVECD_EXEC_START: &str = "/usr/bin/repovecd";
const REPOVEC_MCPD_EXEC_START: &str = "/usr/bin/repovec-mcpd";

/// Validates the repository's checked-in repovec systemd units.
///
/// # Errors
///
/// Returns [`SystemdUnitError`] when a checked-in systemd unit no longer
/// satisfies the appliance contract.
///
/// # Examples
///
/// ```
/// use repovec_core::appliance::systemd_units::validate_checked_in_systemd_units;
///
/// validate_checked_in_systemd_units().expect("the checked-in units remain valid");
/// ```
pub fn validate_checked_in_systemd_units() -> Result<(), SystemdUnitError> {
    validate_systemd_units(
        CHECKED_IN_REPOVEC_TARGET,
        CHECKED_IN_REPOVECD_SERVICE,
        CHECKED_IN_REPOVEC_MCPD_SERVICE,
    )
}

/// Validates repovec systemd unit contents against the appliance contract.
///
/// # Errors
///
/// Returns [`SystemdUnitError`] describing the first contract violation.
///
/// # Examples
///
/// ```
/// use repovec_core::appliance::systemd_units::validate_systemd_units;
///
/// let target = "\
/// [Unit]
/// Wants=qdrant.service repovecd.service repovec-mcpd.service cloudflared.service
///
/// [Install]
/// WantedBy=multi-user.target
/// ";
/// let repovecd = "\
/// [Unit]
/// Requires=qdrant.service
/// After=qdrant.service
///
/// [Service]
/// User=repovec
/// Group=repovec
/// WorkingDirectory=/var/lib/repovec
/// Environment=HOME=/var/lib/repovec
/// ExecStart=/usr/bin/repovecd
/// ";
/// let mcpd = "\
/// [Unit]
/// Requires=qdrant.service repovecd.service
/// After=qdrant.service repovecd.service
///
/// [Service]
/// User=repovec
/// Group=repovec
/// WorkingDirectory=/var/lib/repovec
/// Environment=HOME=/var/lib/repovec
/// ExecStart=/usr/bin/repovec-mcpd
/// ";
///
/// validate_systemd_units(target, repovecd, mcpd)
///     .expect("the inline units should satisfy the contract");
/// ```
pub fn validate_systemd_units(
    target_contents: &str,
    repovecd_contents: &str,
    mcpd_contents: &str,
) -> Result<(), SystemdUnitError> {
    let target_unit = ParsedUnit::parse(target_contents)?;
    let repovecd_unit = ParsedUnit::parse(repovecd_contents)?;
    let mcpd_unit = ParsedUnit::parse(mcpd_contents)?;

    validate_target_unit(&target_unit)?;
    validate_service_unit(
        &repovecd_unit,
        REPOVECD_SERVICE,
        &[QDRANT_SERVICE],
        REPOVECD_EXEC_START,
    )?;
    validate_service_unit(
        &mcpd_unit,
        REPOVEC_MCPD_SERVICE,
        &[QDRANT_SERVICE, REPOVECD_SERVICE],
        REPOVEC_MCPD_EXEC_START,
    )
}

fn validate_target_unit(target_unit: &ParsedUnit) -> Result<(), SystemdUnitError> {
    for target_dependency in
        [QDRANT_SERVICE, REPOVECD_SERVICE, REPOVEC_MCPD_SERVICE, CLOUDFLARED_SERVICE]
    {
        validate_dependency(target_unit, "repovec.target", "Wants", target_dependency)?;
    }

    validate_single_value(
        target_unit,
        RequiredValue {
            unit: "repovec.target",
            section: INSTALL_SECTION,
            directive: "WantedBy",
            expected: MULTI_USER_TARGET,
        },
    )
}

fn validate_service_unit(
    service_unit: &ParsedUnit,
    unit_name: &'static str,
    dependencies: &[&'static str],
    exec_start: &'static str,
) -> Result<(), SystemdUnitError> {
    for service_dependency in dependencies {
        validate_dependency(service_unit, unit_name, "Requires", service_dependency)?;
        validate_dependency(service_unit, unit_name, "After", service_dependency)?;
    }

    validate_single_value(
        service_unit,
        RequiredValue {
            unit: unit_name,
            section: SERVICE_SECTION,
            directive: "User",
            expected: SERVICE_USER,
        },
    )?;
    validate_single_value(
        service_unit,
        RequiredValue {
            unit: unit_name,
            section: SERVICE_SECTION,
            directive: "Group",
            expected: SERVICE_GROUP,
        },
    )?;
    validate_single_value(
        service_unit,
        RequiredValue {
            unit: unit_name,
            section: SERVICE_SECTION,
            directive: "WorkingDirectory",
            expected: WORKING_DIRECTORY,
        },
    )?;
    validate_single_value(
        service_unit,
        RequiredValue {
            unit: unit_name,
            section: SERVICE_SECTION,
            directive: "Environment",
            expected: HOME_ENVIRONMENT,
        },
    )?;
    validate_exec_start(service_unit, unit_name, exec_start)
}

fn validate_dependency(
    parsed_unit: &ParsedUnit,
    unit_name: &'static str,
    directive: &'static str,
    expected_dependency: &'static str,
) -> Result<(), SystemdUnitError> {
    let dependency_values = parsed_unit.values(UNIT_SECTION, directive);
    if dependency_values.is_empty() {
        return Err(SystemdUnitError::MissingDirective {
            unit: unit_name,
            section: UNIT_SECTION,
            directive,
        });
    }

    if dependency_values
        .iter()
        .flat_map(|dependency_value| dependency_value.split_whitespace())
        .any(|dependency| dependency == expected_dependency)
    {
        return Ok(());
    }

    Err(SystemdUnitError::MissingDependency {
        unit: unit_name,
        directive,
        dependency: expected_dependency,
    })
}

fn validate_single_value(
    parsed_unit: &ParsedUnit,
    required_value: RequiredValue,
) -> Result<(), SystemdUnitError> {
    let values = parsed_unit.values(required_value.section, required_value.directive);
    let Some(value) = values.first() else {
        return Err(SystemdUnitError::MissingDirective {
            unit: required_value.unit,
            section: required_value.section,
            directive: required_value.directive,
        });
    };

    if value == required_value.expected {
        Ok(())
    } else {
        Err(SystemdUnitError::IncorrectDirective {
            unit: required_value.unit,
            directive: required_value.directive,
            expected: required_value.expected,
            actual: value.clone(),
        })
    }
}

fn validate_exec_start(
    parsed_unit: &ParsedUnit,
    unit_name: &'static str,
    expected: &'static str,
) -> Result<(), SystemdUnitError> {
    let values = parsed_unit.values(SERVICE_SECTION, "ExecStart");
    let Some(value) = values.first() else {
        return Err(SystemdUnitError::MissingDirective {
            unit: unit_name,
            section: SERVICE_SECTION,
            directive: "ExecStart",
        });
    };

    if value == expected {
        Ok(())
    } else {
        Err(SystemdUnitError::IncorrectExecStart {
            unit: unit_name,
            expected,
            actual: value.clone(),
        })
    }
}

#[derive(Clone, Copy)]
struct RequiredValue {
    unit: &'static str,
    section: &'static str,
    directive: &'static str,
    expected: &'static str,
}
