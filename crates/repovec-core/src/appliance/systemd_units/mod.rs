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
//!   by `repovec.target`, runs `systemd-sysusers` then `systemd-tmpfiles` in
//!   that exact order, and is ordered before the Qdrant API-key oneshot,
//!   Qdrant, and both daemons. It returns to inactive after success so a later
//!   target restart reruns the idempotent provisioning commands.
//! - A requirement, not merely an ordering, from each provisioned consumer back
//!   to the oneshot: `repovecd` and `repovec-mcpd` must `Requires=`
//!   `repovec-provision.service`. `Before=` in the oneshot only sequences the
//!   start jobs, so without the reverse requirement a failed provisioning run
//!   would still leave the daemons starting against an unprovisioned host.
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

mod assets;
mod error;
mod parsed;
mod startup;
mod validate;

#[cfg(test)]
mod tests;

// The contract vocabulary is re-exported under the module's own name so the
// validators, their tests, and the public API all keep referring to a single
// path. `assets.rs` owns the definitions.
pub use assets::{
    CHECKED_IN_REPOVEC_GREPAI_TEMPLATE_PATH, CHECKED_IN_REPOVEC_MCPD_SERVICE_PATH,
    CHECKED_IN_REPOVEC_PROVISION_SERVICE_PATH, CHECKED_IN_REPOVEC_TARGET_PATH,
    CHECKED_IN_REPOVECD_SERVICE_PATH, checked_in_repovec_grepai_template,
    checked_in_repovec_mcpd_service, checked_in_repovec_provision_service,
    checked_in_repovec_target, checked_in_repovecd_service,
};
use assets::{
    GREPAI_BOOLEAN_HARDENING_DIRECTIVES, GREPAI_HARDENING_DIRECTIVES,
    GREPAI_RESTRICT_ADDRESS_FAMILIES, GREPAI_WORKING_DIRECTORY, INSTALL_SECTION,
    PROVISION_EXEC_SEQUENCE, QDRANT_API_KEY_SERVICE, QDRANT_CONTAINER, QDRANT_CONTAINER_SERVICE,
    QDRANT_SERVICE, REPOVEC_GREPAI_TEMPLATE_UNIT, REPOVEC_MCPD_UNIT, REPOVEC_PROVISION_SERVICE,
    REPOVECD_UNIT, SERVICE_GROUP, SERVICE_HOME_ENVIRONMENT, SERVICE_SECTION, SERVICE_USER,
    SERVICE_WORKING_DIRECTORY, SYSUSERS_SERVICE, TARGET_UNIT, UNIT_SECTION,
};
pub use error::SystemdUnitError;
use parsed::ParsedUnit;
pub use startup::{run_startup_validation, validate_and_trace_checked_in_units};
use validate::{
    validate_grepai_template, validate_mcpd, validate_provision_service, validate_repovecd,
    validate_target,
};

/// The five unit texts that make up one appliance unit set.
///
/// The validators need every unit at once, and the mutation-test fixture swaps
/// them independently, so the set travels as one named value rather than as five
/// positional arguments that are easy to transpose at a call site.
#[derive(Clone, Copy, Debug)]
pub(crate) struct UnitTexts<'a> {
    /// The target unit that pulls the appliance services in.
    pub(crate) target: &'a str,
    /// The control-plane daemon unit.
    pub(crate) repovecd: &'a str,
    /// The MCP daemon unit.
    pub(crate) mcpd: &'a str,
    /// The per-repository indexer template unit.
    pub(crate) grepai_template: &'a str,
    /// The provisioning oneshot unit.
    pub(crate) provision: &'a str,
}

impl UnitTexts<'_> {
    /// The checked-in unit set, borrowed from the embedded assets.
    pub(crate) const fn checked_in() -> Self {
        Self {
            target: checked_in_repovec_target(),
            repovecd: checked_in_repovecd_service(),
            mcpd: checked_in_repovec_mcpd_service(),
            grepai_template: checked_in_repovec_grepai_template(),
            provision: checked_in_repovec_provision_service(),
        }
    }
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
    validate_systemd_units_with_grepai_template_and_provision(UnitTexts::checked_in())
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
/// Requires=repovec-provision.service
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
/// Requires=repovec-provision.service
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
/// Requires=repovec-provision.service
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
/// Requires=repovec-provision.service
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
    validate_systemd_units_with_grepai_template_and_provision(UnitTexts {
        target: repovec_target,
        repovecd: repovecd_service,
        mcpd: repovec_mcpd_service,
        grepai_template: repovec_grepai_template,
        provision: checked_in_repovec_provision_service(),
    })
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
pub(crate) fn validate_systemd_units_with_grepai_template_and_provision(
    unit_texts: UnitTexts<'_>,
) -> Result<(), SystemdUnitError> {
    validate_systemd_units(unit_texts.target, unit_texts.repovecd, unit_texts.mcpd)?;
    let template = ParsedUnit::parse(REPOVEC_GREPAI_TEMPLATE_UNIT, unit_texts.grepai_template)?;

    validate_grepai_template(&template)?;
    let provision = ParsedUnit::parse(REPOVEC_PROVISION_SERVICE, unit_texts.provision)?;

    validate_provision_service(&provision)
}
