//! Validation helpers for the checked-in repovec systemd unit assets.
//!
//! This module belongs to [`crate::appliance`]. It embeds the repovec appliance
//! systemd unit files at compile time with [`include_str!`] and exposes the
//! static validation surface for the service-layout contract used by the
//! appliance packaging and daemon startup paths.
//!
//! ## Validation Entry Points
//!
//! - [`validate_checked_in_systemd_units`] validates the three embedded unit
//!   assets shipped in the repository.
//! - [`validate_and_trace_checked_in_units`] validates the checked-in unit set
//!   and emits the daemon startup success trace.
//! - [`validate_systemd_units`] validates caller-supplied unit text. Use it in
//!   tests or tooling that needs to analyse unit contents sourced outside the
//!   checked-in files.
//!
//! The validators return `Ok(())`, or [`SystemdUnitError`] for the first
//! violation found.
//!
//! ## Contract Scope
//!
//! The validators perform static analysis on systemd unit text. They do not
//! invoke `systemctl`, inspect the live systemd manager, or read unit files from
//! `/etc/systemd/`. The checked-in validation path reads no filesystem state at
//! runtime beyond the compile-time [`include_str!`] asset embedding.
//!
//! The service-layout contract enforces:
//!
//! - Required section headers for the relevant unit type: `[Unit]`, `[Service]`,
//!   and `[Install]`.
//! - Dependency and ordering directives: `Wants=`, `Requires=`, `After=`, and
//!   `WantedBy=`.
//! - Rejection of Quadlet-derived Qdrant dependency names such as
//!   `qdrant.container` and `qdrant.container.service`.
//! - `ExecStart=` executable paths for `repovecd` and `repovec-mcpd`.
//! - `[Service]` identity and runtime-directory directives: `User=`, `Group=`,
//!   `WorkingDirectory=`, and `Environment=HOME=`.
//!
//! The validators do not verify that referenced binaries, users, groups,
//! directories, or services exist on the host.
//!
//! ## Daemon Startup Contract
//!
//! The daemon binaries (`repovecd` and `repovec-mcpd`) call
//! [`validate_and_trace_checked_in_units`] as the first substantive action in
//! `main()`. Any [`SystemdUnitError`] is fatal at startup: the daemon logs the
//! violation with `tracing::error!` and exits with code 1.

mod error;
mod parsed;
mod startup;

#[cfg(test)]
mod tests;

pub use error::SystemdUnitError;
pub use startup::{run_startup_validation, validate_and_trace_checked_in_units};
use parsed::ParsedUnit;

const CHECKED_IN_REPOVEC_TARGET: &str =
    include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/../../packaging/systemd/repovec.target"));
const CHECKED_IN_REPOVECD_SERVICE: &str =
    include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/../../packaging/systemd/repovecd.service"));
const CHECKED_IN_REPOVEC_MCPD_SERVICE: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../packaging/systemd/repovec-mcpd.service"
));

const CHECKED_IN_REPOVEC_GREPAI_TEMPLATE: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../packaging/systemd/repovec-grepai@.service"
));
/// The repository path of the checked-in `repovec.target` unit.
pub const CHECKED_IN_REPOVEC_TARGET_PATH: &str = "packaging/systemd/repovec.target";
/// The repository path of the checked-in `repovecd.service` unit.
pub const CHECKED_IN_REPOVECD_SERVICE_PATH: &str = "packaging/systemd/repovecd.service";
/// The repository path of the checked-in `repovec-mcpd.service` unit.
pub const CHECKED_IN_REPOVEC_MCPD_SERVICE_PATH: &str = "packaging/systemd/repovec-mcpd.service";

/// The repository path of the checked-in `repovec-grepai@.service` template.
pub const CHECKED_IN_REPOVEC_GREPAI_TEMPLATE_PATH: &str =
    "packaging/systemd/repovec-grepai@.service";
const TARGET_UNIT: &str = "repovec.target";
const REPOVECD_UNIT: &str = "repovecd.service";
const REPOVEC_MCPD_UNIT: &str = "repovec-mcpd.service";
const REPOVEC_GREPAI_TEMPLATE_UNIT: &str = "repovec-grepai@.service";
const UNIT_SECTION: &str = "Unit";
const SERVICE_SECTION: &str = "Service";
const INSTALL_SECTION: &str = "Install";
const QDRANT_SERVICE: &str = "qdrant.service";
const QDRANT_CONTAINER: &str = "qdrant.container";
const QDRANT_CONTAINER_SERVICE: &str = "qdrant.container.service";
const SERVICE_USER: &str = "repovec";
const SERVICE_GROUP: &str = "repovec";
const SERVICE_WORKING_DIRECTORY: &str = "/var/lib/repovec";
const GREPAI_WORKING_DIRECTORY: &str = "/var/lib/repovec/worktrees/%I";
const SERVICE_HOME_ENVIRONMENT: &str = "HOME=/var/lib/repovec";

/// Returns the repository's checked-in `repovec.target` source.
///
/// # Examples
///
/// ```
/// use repovec_core::appliance::systemd_units::checked_in_repovec_target;
///
/// assert!(checked_in_repovec_target().contains("[Unit]"));
/// ```
#[must_use]
pub const fn checked_in_repovec_target() -> &'static str { CHECKED_IN_REPOVEC_TARGET }

/// Returns the repository's checked-in `repovecd.service` source.
///
/// # Examples
///
/// ```
/// use repovec_core::appliance::systemd_units::checked_in_repovecd_service;
///
/// assert!(checked_in_repovecd_service().contains("ExecStart=/usr/bin/repovecd"));
/// ```
#[must_use]
pub const fn checked_in_repovecd_service() -> &'static str { CHECKED_IN_REPOVECD_SERVICE }

/// Returns the repository's checked-in `repovec-mcpd.service` source.
///
/// # Examples
///
/// ```
/// use repovec_core::appliance::systemd_units::checked_in_repovec_mcpd_service;
///
/// assert!(checked_in_repovec_mcpd_service().contains("ExecStart=/usr/bin/repovec-mcpd"));
/// ```
#[must_use]
pub const fn checked_in_repovec_mcpd_service() -> &'static str { CHECKED_IN_REPOVEC_MCPD_SERVICE }

pub const fn checked_in_repovec_grepai_template() -> &'static str {
    CHECKED_IN_REPOVEC_GREPAI_TEMPLATE
}
/// Validates the repository's checked-in repovec systemd unit definitions.
///
/// # Errors
///
/// Returns [`SystemdUnitError`] when a checked-in unit no longer satisfies the
/// appliance service-layout contract.
///
/// # Examples
///
/// ```
/// use repovec_core::appliance::systemd_units::validate_checked_in_systemd_units;
///
/// validate_checked_in_systemd_units().expect("the checked-in units remain valid");
/// ```
pub fn validate_checked_in_systemd_units() -> Result<(), SystemdUnitError> {
    validate_systemd_units_with_grepai_template(
        checked_in_repovec_target(),
        checked_in_repovecd_service(),
        checked_in_repovec_mcpd_service(),
        checked_in_repovec_grepai_template(),
    )
}

/// Validates arbitrary repovec systemd unit contents against the appliance contract.
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
/// validate_systemd_units(target, repovecd, mcpd).expect("inline units satisfy the contract");
/// ```
pub fn validate_systemd_units(
    repovec_target: &str,
    repovecd_service: &str,
    repovec_mcpd_service: &str,
) -> Result<(), SystemdUnitError> {
    let target = ParsedUnit::parse(TARGET_UNIT, repovec_target)?;
    let repovecd = ParsedUnit::parse(REPOVECD_UNIT, repovecd_service)?;
    let mcpd = ParsedUnit::parse(REPOVEC_MCPD_UNIT, repovec_mcpd_service)?;

    validate_target(&target)?;
    validate_repovecd(&repovecd)?;
    validate_mcpd(&mcpd)
}

pub fn validate_systemd_units_with_grepai_template(
    repovec_target: &str,
    repovecd_service: &str,
    repovec_mcpd_service: &str,
    repovec_grepai_template: &str,
) -> Result<(), SystemdUnitError> {
    validate_systemd_units(repovec_target, repovecd_service, repovec_mcpd_service)?;
    let template = ParsedUnit::parse(REPOVEC_GREPAI_TEMPLATE_UNIT, repovec_grepai_template)?;

    validate_grepai_template(&template)
}
fn validate_target(target: &ParsedUnit) -> Result<(), SystemdUnitError> {
    target.require_section(UNIT_SECTION)?;
    target.require_section(INSTALL_SECTION)?;
    target.require_dependency(INSTALL_SECTION, "WantedBy", "multi-user.target")?;
    target.require_dependency(UNIT_SECTION, "Wants", QDRANT_SERVICE)?;
    target.require_dependency(UNIT_SECTION, "Wants", REPOVECD_UNIT)?;
    target.require_dependency(UNIT_SECTION, "Wants", REPOVEC_MCPD_UNIT)?;
    target.require_dependency(UNIT_SECTION, "Wants", "cloudflared.service")
}

fn validate_repovecd(repovecd: &ParsedUnit) -> Result<(), SystemdUnitError> {
    repovecd.require_section(UNIT_SECTION)?;
    repovecd.require_section(SERVICE_SECTION)?;
    repovecd.require_dependency(UNIT_SECTION, "Requires", QDRANT_SERVICE)?;
    repovecd.require_dependency(UNIT_SECTION, "After", QDRANT_SERVICE)?;
    repovecd.require_exec_start("/usr/bin/repovecd")?;
    repovecd.require_service_directive("User", SERVICE_USER)?;
    repovecd.require_service_directive("Group", SERVICE_GROUP)?;
    repovecd.require_service_directive("WorkingDirectory", SERVICE_WORKING_DIRECTORY)?;
    repovecd.require_service_environment(SERVICE_HOME_ENVIRONMENT)
}

fn validate_mcpd(mcpd: &ParsedUnit) -> Result<(), SystemdUnitError> {
    mcpd.require_section(UNIT_SECTION)?;
    mcpd.require_section(SERVICE_SECTION)?;
    mcpd.require_dependency(UNIT_SECTION, "Requires", QDRANT_SERVICE)?;
    mcpd.require_dependency(UNIT_SECTION, "Requires", REPOVECD_UNIT)?;
    mcpd.require_dependency(UNIT_SECTION, "After", QDRANT_SERVICE)?;
    mcpd.require_dependency(UNIT_SECTION, "After", REPOVECD_UNIT)?;
    mcpd.require_exec_start("/usr/bin/repovec-mcpd")?;
    mcpd.require_service_directive("User", SERVICE_USER)?;
    mcpd.require_service_directive("Group", SERVICE_GROUP)?;
    mcpd.require_service_directive("WorkingDirectory", SERVICE_WORKING_DIRECTORY)?;
    mcpd.require_service_environment(SERVICE_HOME_ENVIRONMENT)
}

fn validate_grepai_template(template: &ParsedUnit) -> Result<(), SystemdUnitError> {
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
    template.require_service_directive("StandardOutput", "journal")?;
    template.require_service_directive("StandardError", "journal")
}
