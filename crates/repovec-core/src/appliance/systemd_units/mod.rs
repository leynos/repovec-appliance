//! Validation helpers for the checked-in repovec systemd unit assets.
//!
//! This module belongs to [`crate::appliance`]. It embeds the repovec appliance
//! systemd unit files at compile time with [`include_str!`] and exposes the
//! static validation surface for the service-layout contract used by the
//! appliance packaging and daemon startup paths.
//!
//! ## Validation Entry Points
//!
//! - [`validate_checked_in_systemd_units`] validates the five embedded unit
//!   assets shipped in the repository.
//! - [`validate_and_trace_checked_in_units`] validates the checked-in unit set
//!   and emits the daemon startup success trace.
//! - [`validate_systemd_units`] validates caller-supplied unit text. Use it in
//!   tests or tooling that needs to analyse unit contents sourced outside the
//!   checked-in files.
//! - [`validate_systemd_units_with_grepai_template`] validates caller-supplied
//!   target, daemon, and grepai template text against the full appliance
//!   service-layout contract.
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
//! - `ExecStart=` executable paths for `repovecd`, `repovec-mcpd`, and
//!   `grepai watch`.
//! - The provisioning oneshot contract: `repovec-provision.service` is wanted
//!   by `repovec.target`, runs `systemd-sysusers` then `systemd-tmpfiles`, and
//!   is ordered before the Qdrant API-key oneshot, Qdrant, and both daemons.
//! - `[Service]` identity and runtime-directory directives: `User=`, `Group=`,
//!   `WorkingDirectory=`, and `Environment=HOME=`.
//! - Grepai template directives that bind future instances to
//!   `repovec.target`, use `WorkingDirectory=/var/lib/repovec/worktrees/%I`,
//!   and keep stdout and stderr in journald.
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
mod validate;

#[cfg(test)]
mod tests;

pub use error::SystemdUnitError;
use parsed::ParsedUnit;
pub use startup::{run_startup_validation, validate_and_trace_checked_in_units};
use validate::{
    validate_grepai_template, validate_mcpd, validate_provision_service, validate_repovecd,
    validate_target,
};

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
const CHECKED_IN_REPOVEC_PROVISION_SERVICE: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../packaging/systemd/repovec-provision.service"
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
/// The repository path of the checked-in `repovec-provision.service` unit.
pub const CHECKED_IN_REPOVEC_PROVISION_SERVICE_PATH: &str =
    "packaging/systemd/repovec-provision.service";
const TARGET_UNIT: &str = "repovec.target";
const REPOVECD_UNIT: &str = "repovecd.service";
const REPOVEC_MCPD_UNIT: &str = "repovec-mcpd.service";
const REPOVEC_GREPAI_TEMPLATE_UNIT: &str = "repovec-grepai@.service";
const REPOVEC_PROVISION_SERVICE: &str = "repovec-provision.service";
const UNIT_SECTION: &str = "Unit";
const SERVICE_SECTION: &str = "Service";
const INSTALL_SECTION: &str = "Install";
const QDRANT_SERVICE: &str = "qdrant.service";
const QDRANT_CONTAINER: &str = "qdrant.container";
const QDRANT_CONTAINER_SERVICE: &str = "qdrant.container.service";
const QDRANT_API_KEY_SERVICE: &str = "repovec-qdrant-api-key.service";
const SYSUSERS_SERVICE: &str = "systemd-sysusers.service";
const SYSUSERS_EXEC: &str = "/usr/bin/systemd-sysusers /usr/lib/sysusers.d/repovec.conf";
const TMPFILES_EXEC: &str = "/usr/bin/systemd-tmpfiles --create /usr/lib/tmpfiles.d/repovec.conf";
const SERVICE_USER: &str = "repovec";
const SERVICE_GROUP: &str = "repovec";
const SERVICE_WORKING_DIRECTORY: &str = "/var/lib/repovec";
const GREPAI_WORKING_DIRECTORY: &str = "/var/lib/repovec/worktrees/%I";
const SERVICE_HOME_ENVIRONMENT: &str = "HOME=/var/lib/repovec";
const GREPAI_HARDENING_DIRECTIVES: [(&str, &str); 15] = [
    ("NoNewPrivileges", "yes"),
    ("PrivateTmp", "yes"),
    ("ProtectSystem", "full"),
    ("ProtectHome", "read-only"),
    ("PrivateDevices", "yes"),
    ("DevicePolicy", "closed"),
    ("LockPersonality", "yes"),
    ("ProtectClock", "yes"),
    ("ProtectControlGroups", "yes"),
    ("ProtectHostname", "yes"),
    ("ProtectKernelLogs", "yes"),
    ("ProtectKernelModules", "yes"),
    ("ProtectKernelTunables", "yes"),
    ("ProtectProc", "invisible"),
    ("ProcSubset", "pid"),
];
const GREPAI_BOOLEAN_HARDENING_DIRECTIVES: [(&str, &str); 3] =
    [("RestrictNamespaces", "yes"), ("RestrictRealtime", "yes"), ("RestrictSUIDSGID", "yes")];
const GREPAI_RESTRICT_ADDRESS_FAMILIES: &str = "AF_UNIX AF_INET AF_INET6";

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

/// Returns the repository's checked-in `repovec-grepai@.service` template.
///
/// # Examples
///
/// ```
/// use repovec_core::appliance::systemd_units::checked_in_repovec_grepai_template;
///
/// assert!(checked_in_repovec_grepai_template().contains("ExecStart=/usr/bin/grepai watch"));
/// ```
#[must_use]
pub const fn checked_in_repovec_grepai_template() -> &'static str {
    CHECKED_IN_REPOVEC_GREPAI_TEMPLATE
}

/// Returns the repository's checked-in `repovec-provision.service` source.
///
/// # Examples
///
/// ```
/// use repovec_core::appliance::systemd_units::checked_in_repovec_provision_service;
///
/// assert!(checked_in_repovec_provision_service().contains("ExecStart=/usr/bin/systemd-sysusers"));
/// ```
#[must_use]
pub const fn checked_in_repovec_provision_service() -> &'static str {
    CHECKED_IN_REPOVEC_PROVISION_SERVICE
}

/// Validates the repository's checked-in repovec systemd unit definitions.
///
/// This checks the embedded appliance target, daemon services, and grepai
/// indexer template against the full static service-layout contract.
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
    validate_systemd_units_with_grepai_template_and_provision(
        checked_in_repovec_target(),
        checked_in_repovecd_service(),
        checked_in_repovec_mcpd_service(),
        checked_in_repovec_grepai_template(),
        checked_in_repovec_provision_service(),
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
/// Wants=qdrant.service repovecd.service repovec-mcpd.service cloudflared.service repovec-provision.service
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

/// Validates arbitrary repovec systemd units, including the grepai template.
///
/// Use this entry point when tests or tooling need to validate the complete
/// target, daemon, and per-repository indexer template contract from supplied
/// unit text rather than the checked-in embedded assets.
///
/// # Errors
///
/// Returns [`SystemdUnitError`] describing the first contract violation.
///
/// # Examples
///
/// ```
/// use repovec_core::appliance::systemd_units::{
///     checked_in_repovec_grepai_template, validate_systemd_units_with_grepai_template,
/// };
///
/// let target = "\
/// [Unit]
/// Wants=qdrant.service repovecd.service repovec-mcpd.service cloudflared.service repovec-provision.service
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
/// validate_systemd_units_with_grepai_template(
///     target,
///     repovecd,
///     mcpd,
///     checked_in_repovec_grepai_template(),
/// )
/// .expect("inline units and the checked-in template satisfy the contract");
/// ```
pub fn validate_systemd_units_with_grepai_template(
    repovec_target: &str,
    repovecd_service: &str,
    repovec_mcpd_service: &str,
    repovec_grepai_template: &str,
) -> Result<(), SystemdUnitError> {
    validate_systemd_units_with_grepai_template_and_provision(
        repovec_target,
        repovecd_service,
        repovec_mcpd_service,
        repovec_grepai_template,
        checked_in_repovec_provision_service(),
    )
}

/// Validates arbitrary repovec systemd units, including the grepai template and
/// the provisioning oneshot.
///
/// This is a crate-private test seam: the public entry points validate the
/// provisioning oneshot from the checked-in asset, while the mutation-test
/// fixture needs to supply a mutated provision unit. Keep it out of the public
/// API surface.
///
/// # Errors
///
/// Returns [`SystemdUnitError`] describing the first contract violation.
#[expect(
    clippy::too_many_arguments,
    reason = "crate-private test seam: the mutation-test fixture supplies \
              five independent unit texts (target, repovecd, mcpd, grepai, provision)"
)]
pub(crate) fn validate_systemd_units_with_grepai_template_and_provision(
    repovec_target: &str,
    repovecd_service: &str,
    repovec_mcpd_service: &str,
    repovec_grepai_template: &str,
    repovec_provision_service: &str,
) -> Result<(), SystemdUnitError> {
    validate_systemd_units(repovec_target, repovecd_service, repovec_mcpd_service)?;
    let template = ParsedUnit::parse(REPOVEC_GREPAI_TEMPLATE_UNIT, repovec_grepai_template)?;

    validate_grepai_template(&template)?;
    let provision = ParsedUnit::parse(REPOVEC_PROVISION_SERVICE, repovec_provision_service)?;

    validate_provision_service(&provision)
}
