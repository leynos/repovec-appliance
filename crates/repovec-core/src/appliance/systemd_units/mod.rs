//! Validation helpers for the checked-in repovec systemd unit assets.
//!
//! This module belongs to [`crate::appliance`]. It embeds the repovec appliance
//! systemd unit files at compile time with [`include_str!`] and exposes the
//! static validation surface for the service-layout contract used by the
//! appliance packaging and daemon startup paths.
//!
//! ## Public Accessors
//!
//! - [`checked_in_repovec_target`] returns the embedded `repovec.target` text.
//! - [`checked_in_repovecd_service`] returns the embedded `repovecd.service`
//!   text.
//! - [`checked_in_repovec_mcpd_service`] returns the embedded
//!   `repovec-mcpd.service` text.
//!
//! ## Validation Entry Points
//!
//! - [`validate_checked_in_systemd_units`] validates the three embedded unit
//!   assets shipped in the repository. Use it when the checked-in appliance
//!   contract must be proved before a daemon starts.
//! - [`validate_systemd_units`] validates caller-supplied unit text. Use it in
//!   tests or tooling that needs to analyse unit contents sourced outside the
//!   checked-in files.
//!
//! Both entry points return `Ok(())` when the unit set satisfies the contract,
//! or [`SystemdUnitError`] describing the first violation found. The
//! [`SystemdUnitError`] type is defined in this module's private `error`
//! submodule and re-exported here for callers.
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
//! [`validate_checked_in_systemd_units`] as the first substantive action in
//! `main()`. Any [`SystemdUnitError`] is fatal at startup: the daemon logs the
//! violation with `tracing::error!` and exits with code 1.

mod error;

#[cfg(test)]
mod tests;

use std::collections::{BTreeMap, BTreeSet};

pub use error::SystemdUnitError;

const CHECKED_IN_REPOVEC_TARGET: &str =
    include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/../../packaging/systemd/repovec.target"));
const CHECKED_IN_REPOVECD_SERVICE: &str =
    include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/../../packaging/systemd/repovecd.service"));
const CHECKED_IN_REPOVEC_MCPD_SERVICE: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../packaging/systemd/repovec-mcpd.service"
));

/// The repository path of the checked-in `repovec.target` unit.
pub const CHECKED_IN_REPOVEC_TARGET_PATH: &str = "packaging/systemd/repovec.target";
/// The repository path of the checked-in `repovecd.service` unit.
pub const CHECKED_IN_REPOVECD_SERVICE_PATH: &str = "packaging/systemd/repovecd.service";
/// The repository path of the checked-in `repovec-mcpd.service` unit.
pub const CHECKED_IN_REPOVEC_MCPD_SERVICE_PATH: &str = "packaging/systemd/repovec-mcpd.service";

const TARGET_UNIT: &str = "repovec.target";
const REPOVECD_UNIT: &str = "repovecd.service";
const REPOVEC_MCPD_UNIT: &str = "repovec-mcpd.service";
const UNIT_SECTION: &str = "Unit";
const SERVICE_SECTION: &str = "Service";
const INSTALL_SECTION: &str = "Install";
const QDRANT_SERVICE: &str = "qdrant.service";
const QDRANT_CONTAINER: &str = "qdrant.container";
const QDRANT_CONTAINER_SERVICE: &str = "qdrant.container.service";
const SERVICE_USER: &str = "repovec";
const SERVICE_GROUP: &str = "repovec";
const SERVICE_WORKING_DIRECTORY: &str = "/var/lib/repovec";
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
    validate_systemd_units(
        checked_in_repovec_target(),
        checked_in_repovecd_service(),
        checked_in_repovec_mcpd_service(),
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
#[tracing::instrument(skip_all)]
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
    repovecd.require_service_directive("Environment", SERVICE_HOME_ENVIRONMENT)
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
    mcpd.require_service_directive("Environment", SERVICE_HOME_ENVIRONMENT)
}

#[derive(Debug)]
struct ParsedUnit {
    unit: &'static str,
    sections: BTreeMap<String, BTreeMap<String, Vec<String>>>,
}

impl ParsedUnit {
    fn parse(unit: &'static str, contents: &str) -> Result<Self, SystemdUnitError> {
        let mut sections = BTreeMap::<String, BTreeMap<String, Vec<String>>>::new();
        let mut current_section: Option<String> = None;

        for (line_index, raw_line) in contents.lines().enumerate() {
            let line_number = line_index + 1;
            let line = raw_line.trim();
            if is_ignored_line(line) {
                continue;
            }

            if let Some(section) = parse_section_header(line) {
                current_section = Some(section.to_owned());
                sections.entry(section.to_owned()).or_default();
                continue;
            }

            let Some((key, value)) = line.split_once('=') else {
                return Err(SystemdUnitError::InvalidLine {
                    unit,
                    line_number,
                    line: line.to_owned(),
                });
            };

            let Some(section) = &current_section else {
                return Err(SystemdUnitError::PropertyBeforeSection {
                    unit,
                    line_number,
                    line: line.to_owned(),
                });
            };

            sections
                .entry(section.clone())
                .or_default()
                .entry(key.trim().to_owned())
                .or_default()
                .push(value.trim().to_owned());
        }

        Ok(Self { unit, sections })
    }

    fn require_section(&self, section: &'static str) -> Result<(), SystemdUnitError> {
        if self.sections.contains_key(section) {
            return Ok(());
        }

        Err(SystemdUnitError::MissingSection { unit: self.unit, section })
    }

    fn require_dependency(
        &self,
        section: &'static str,
        key: &'static str,
        dependency: &'static str,
    ) -> Result<(), SystemdUnitError> {
        let tokens = self.directive_tokens(section, key);
        if let Some(quadlet_dependency) =
            tokens.iter().find(|value| is_qdrant_quadlet_source(value))
        {
            return Err(SystemdUnitError::UsesQuadletSourceDependency {
                unit: self.unit,
                section,
                key,
                dependency: (*quadlet_dependency).clone(),
            });
        }

        if tokens.contains(dependency) {
            return Ok(());
        }

        Err(SystemdUnitError::MissingDependency { unit: self.unit, section, key, dependency })
    }

    fn require_exec_start(&self, expected: &'static str) -> Result<(), SystemdUnitError> {
        let values = self.values(SERVICE_SECTION, "ExecStart");
        if values.first().is_some_and(|actual| actual == expected) && values.len() == 1 {
            return Ok(());
        }

        Err(SystemdUnitError::IncorrectExecStart {
            unit: self.unit,
            expected,
            actual: values.join(","),
        })
    }

    fn require_service_directive(
        &self,
        key: &'static str,
        expected: &'static str,
    ) -> Result<(), SystemdUnitError> {
        let values = self.values(SERVICE_SECTION, key);
        if values.first().is_some_and(|value| value == expected) && values.len() == 1 {
            return Ok(());
        }

        Err(SystemdUnitError::IncorrectServiceDirective {
            unit: self.unit,
            key,
            expected,
            actual: values.join(","),
        })
    }

    fn directive_tokens(&self, section: &str, key: &str) -> BTreeSet<String> {
        self.values(section, key)
            .iter()
            .flat_map(|value| value.split_whitespace())
            .map(ToOwned::to_owned)
            .collect()
    }

    fn values(&self, section: &str, key: &str) -> &[String] {
        self.sections.get(section).and_then(|entries| entries.get(key)).map_or(&[], Vec::as_slice)
    }
}

fn parse_section_header(line: &str) -> Option<&str> { line.strip_prefix('[')?.strip_suffix(']') }

fn is_ignored_line(line: &str) -> bool { line.is_empty() || line.starts_with(['#', ';']) }

fn is_qdrant_quadlet_source(value: &str) -> bool {
    value == QDRANT_CONTAINER || value == QDRANT_CONTAINER_SERVICE
}
