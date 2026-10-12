//! Per-unit contract validators for the checked-in systemd units.
//!
//! The parent [`super::systemd_units`] module owns the public entry points and
//! the compile-time asset embedding; this submodule implements the leaf
//! `validate_*` functions that walk a [`ParsedUnit`] against the appliance
//! service-layout contract. Keeping them here bounds the parent module's size.

use super::{
    GREPAI_BOOLEAN_HARDENING_DIRECTIVES, GREPAI_HARDENING_DIRECTIVES,
    GREPAI_RESTRICT_ADDRESS_FAMILIES, GREPAI_WORKING_DIRECTORY, INSTALL_SECTION,
    PROVISION_EXEC_SEQUENCE, QDRANT_API_KEY_SERVICE, QDRANT_SERVICE, REPOVEC_MCPD_UNIT,
    REPOVEC_PROVISION_SERVICE, REPOVECD_UNIT, SERVICE_GROUP, SERVICE_HOME_ENVIRONMENT,
    SERVICE_SECTION, SERVICE_USER, SERVICE_WORKING_DIRECTORY, SYSUSERS_SERVICE, SystemdUnitError,
    TARGET_UNIT, UNIT_SECTION, parsed::ParsedUnit,
};

/// Checks the parsed target's required sections and dependency tokens.
///
/// For example, all required `Wants=` entries return `Ok(())`; a missing entry returns an error.
pub(super) fn validate_target(target: &ParsedUnit) -> Result<(), SystemdUnitError> {
    target.require_section(UNIT_SECTION)?;
    target.require_section(INSTALL_SECTION)?;
    target.require_dependency(INSTALL_SECTION, "WantedBy", "multi-user.target")?;
    target.require_dependency(UNIT_SECTION, "Wants", QDRANT_SERVICE)?;
    target.require_dependency(UNIT_SECTION, "Wants", REPOVECD_UNIT)?;
    target.require_dependency(UNIT_SECTION, "Wants", REPOVEC_MCPD_UNIT)?;
    target.require_dependency(UNIT_SECTION, "Wants", "cloudflared.service")?;
    target.require_dependency(UNIT_SECTION, "Wants", REPOVEC_PROVISION_SERVICE)
}

/// Checks the provisioning oneshot's ordering, type, and seed commands.
///
/// For example, a `oneshot` that is wanted by `repovec.target` and ordered before Qdrant and both
/// daemons returns `Ok(())`; a missing ordering token returns an error.
pub(super) fn validate_provision_service(provision: &ParsedUnit) -> Result<(), SystemdUnitError> {
    provision.require_section(UNIT_SECTION)?;
    provision.require_section(SERVICE_SECTION)?;
    provision.require_section(INSTALL_SECTION)?;
    provision.require_dependency(INSTALL_SECTION, "WantedBy", TARGET_UNIT)?;
    provision.require_dependency(UNIT_SECTION, "Wants", SYSUSERS_SERVICE)?;
    provision.require_dependency(UNIT_SECTION, "After", SYSUSERS_SERVICE)?;
    provision.require_dependency(UNIT_SECTION, "Before", QDRANT_API_KEY_SERVICE)?;
    provision.require_dependency(UNIT_SECTION, "Before", QDRANT_SERVICE)?;
    provision.require_dependency(UNIT_SECTION, "Before", REPOVECD_UNIT)?;
    provision.require_dependency(UNIT_SECTION, "Before", REPOVEC_MCPD_UNIT)?;
    provision.require_service_directive("Type", "oneshot")?;
    provision.require_service_directive("RemainAfterExit", "yes")?;
    provision.require_exec_start_sequence(PROVISION_EXEC_SEQUENCE)
}

/// Checks a parsed daemon unit's Qdrant dependency and service identity.
///
/// For example, valid `User=`, `Group=`, and Qdrant entries return `Ok(())`; a mismatch returns an
/// error.
pub(super) fn validate_repovecd(repovecd: &ParsedUnit) -> Result<(), SystemdUnitError> {
    repovecd.require_section(UNIT_SECTION)?;
    repovecd.require_section(SERVICE_SECTION)?;
    repovecd.require_dependency(UNIT_SECTION, "Requires", QDRANT_SERVICE)?;
    repovecd.require_dependency(UNIT_SECTION, "Requires", REPOVEC_PROVISION_SERVICE)?;
    repovecd.require_dependency(UNIT_SECTION, "After", QDRANT_SERVICE)?;
    repovecd.require_exec_start("/usr/bin/repovecd")?;
    repovecd.require_service_directive("User", SERVICE_USER)?;
    repovecd.require_service_directive("Group", SERVICE_GROUP)?;
    repovecd.require_service_directive("WorkingDirectory", SERVICE_WORKING_DIRECTORY)?;
    repovecd.require_service_environment(SERVICE_HOME_ENVIRONMENT)
}

/// Checks a parsed MCP unit's dependencies and service settings.
///
/// For example, both required dependencies and matching service values return `Ok(())`; a
/// mismatch returns an error.
pub(super) fn validate_mcpd(mcpd: &ParsedUnit) -> Result<(), SystemdUnitError> {
    mcpd.require_section(UNIT_SECTION)?;
    mcpd.require_section(SERVICE_SECTION)?;
    mcpd.require_dependency(UNIT_SECTION, "Requires", QDRANT_SERVICE)?;
    mcpd.require_dependency(UNIT_SECTION, "Requires", REPOVECD_UNIT)?;
    mcpd.require_dependency(UNIT_SECTION, "Requires", REPOVEC_PROVISION_SERVICE)?;
    mcpd.require_dependency(UNIT_SECTION, "After", QDRANT_SERVICE)?;
    mcpd.require_dependency(UNIT_SECTION, "After", REPOVECD_UNIT)?;
    mcpd.require_exec_start("/usr/bin/repovec-mcpd")?;
    mcpd.require_service_directive("User", SERVICE_USER)?;
    mcpd.require_service_directive("Group", SERVICE_GROUP)?;
    mcpd.require_service_directive("WorkingDirectory", SERVICE_WORKING_DIRECTORY)?;
    mcpd.require_service_environment(SERVICE_HOME_ENVIRONMENT)
}

/// Checks a parsed grepai template's target linkage and service settings.
///
/// For example, valid target dependencies and service directives return `Ok(())`; a mismatch
/// returns an error.
pub(super) fn validate_grepai_template(template: &ParsedUnit) -> Result<(), SystemdUnitError> {
    template.require_section(UNIT_SECTION)?;
    template.require_section(SERVICE_SECTION)?;
    template.require_section(INSTALL_SECTION)?;
    template.require_dependency(UNIT_SECTION, "Requires", QDRANT_SERVICE)?;
    template.require_dependency(UNIT_SECTION, "Requires", REPOVECD_UNIT)?;
    template.require_dependency(UNIT_SECTION, "After", QDRANT_SERVICE)?;
    template.require_dependency(UNIT_SECTION, "After", REPOVECD_UNIT)?;
    template.require_dependency(UNIT_SECTION, "PartOf", TARGET_UNIT)?;
    template.require_dependency(INSTALL_SECTION, "WantedBy", TARGET_UNIT)?;
    template.require_service_directive("Type", "exec")?;
    template.require_service_directive("User", SERVICE_USER)?;
    template.require_service_directive("Group", SERVICE_GROUP)?;
    template.require_service_directive("WorkingDirectory", GREPAI_WORKING_DIRECTORY)?;
    template.require_service_environment(SERVICE_HOME_ENVIRONMENT)?;
    template.require_exec_start("/usr/bin/grepai watch")?;
    template.require_service_directive("Restart", "on-failure")?;
    template.require_service_directive("RestartSec", "5s")?;
    validate_grepai_template_hardening(template)?;
    template.require_service_directive("StandardOutput", "journal")?;
    template.require_service_directive("StandardError", "journal")
}

/// Checks a parsed grepai template against the required hardening values.
///
/// For example, each configured directive at its expected value returns `Ok(())`; a mismatch
/// returns an error.
pub(super) fn validate_grepai_template_hardening(
    template: &ParsedUnit,
) -> Result<(), SystemdUnitError> {
    for (key, expected) in GREPAI_HARDENING_DIRECTIVES {
        template.require_service_directive(key, expected)?;
    }
    for (key, expected) in GREPAI_BOOLEAN_HARDENING_DIRECTIVES {
        template.require_service_directive(key, expected)?;
    }
    template.require_service_directive("RestrictAddressFamilies", GREPAI_RESTRICT_ADDRESS_FAMILIES)
}
