//! Embedded checked-in unit assets and the contract vocabulary they describe.
//!
//! The five unit files shipped in `packaging/systemd/` are embedded at compile
//! time with [`include_str!`], so the validators never read them from the
//! filesystem at runtime. The constants that name the units, sections,
//! dependencies, service identity, and hardening directives live here beside
//! the assets they describe. Keeping them together puts the whole contract
//! vocabulary in one reviewable place instead of scattering it through the
//! validator entry points.

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
/// Name used to identify the appliance target in validation errors.
pub(super) const TARGET_UNIT: &str = "repovec.target";
/// Name of the repovecd service unit.
pub(super) const REPOVECD_UNIT: &str = "repovecd.service";
/// Name of the repovec MCP daemon service unit.
pub(super) const REPOVEC_MCPD_UNIT: &str = "repovec-mcpd.service";
/// Name of the templated grepai service unit.
pub(super) const REPOVEC_GREPAI_TEMPLATE_UNIT: &str = "repovec-grepai@.service";
/// Name of the provisioning oneshot that seeds the appliance accounts and directories.
pub(super) const REPOVEC_PROVISION_SERVICE: &str = "repovec-provision.service";
/// Systemd section containing unit metadata and dependencies.
pub(super) const UNIT_SECTION: &str = "Unit";
/// Systemd section containing service execution settings.
pub(super) const SERVICE_SECTION: &str = "Service";
/// Systemd section containing installation relationships.
pub(super) const INSTALL_SECTION: &str = "Install";
/// Required host-managed Qdrant service dependency.
pub(super) const QDRANT_SERVICE: &str = "qdrant.service";
/// Quadlet container name rejected as a service dependency.
pub(super) const QDRANT_CONTAINER: &str = "qdrant.container";
/// Quadlet-generated service name rejected as a dependency.
pub(super) const QDRANT_CONTAINER_SERVICE: &str = "qdrant.container.service";
/// Oneshot that writes the Qdrant API key before the container starts.
pub(super) const QDRANT_API_KEY_SERVICE: &str = "repovec-qdrant-api-key.service";
/// Systemd's own account-management unit, ordered before provisioning runs.
pub(super) const SYSUSERS_SERVICE: &str = "systemd-sysusers.service";
/// Provisioning command that creates the `repovec` account and group.
pub(super) const SYSUSERS_EXEC: &str = "/usr/bin/systemd-sysusers /usr/lib/sysusers.d/repovec.conf";
/// Provisioning command that creates the appliance directory tree.
pub(super) const TMPFILES_EXEC: &str =
    "/usr/bin/systemd-tmpfiles --create /usr/lib/tmpfiles.d/repovec.conf";
/// The provisioning commands, in the order `systemd` must run them.
///
/// `systemd-sysusers` creates the `repovec` account before `systemd-tmpfiles`
/// resolves the `repovec:repovec` ownership of the directory tree (risk R-2), so
/// the order is part of the contract, not an incidental detail.
pub(super) const PROVISION_EXEC_SEQUENCE: &[&str] = &[SYSUSERS_EXEC, TMPFILES_EXEC];
/// Account that owns the appliance services.
pub(super) const SERVICE_USER: &str = "repovec";
/// Group assigned to the appliance services.
pub(super) const SERVICE_GROUP: &str = "repovec";
/// Working directory required for daemon services.
pub(super) const SERVICE_WORKING_DIRECTORY: &str = "/var/lib/repovec";
/// Instance-specific working directory required for grepai.
pub(super) const GREPAI_WORKING_DIRECTORY: &str = "/var/lib/repovec/worktrees/%I";
/// HOME environment entry required for appliance services.
pub(super) const SERVICE_HOME_ENVIRONMENT: &str = "HOME=/var/lib/repovec";
/// Exact hardening directives required by each grepai template instance.
pub(super) const GREPAI_HARDENING_DIRECTIVES: [(&str, &str); 15] = [
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
/// Boolean hardening directives required by the grepai template.
pub(super) const GREPAI_BOOLEAN_HARDENING_DIRECTIVES: [(&str, &str); 3] =
    [("RestrictNamespaces", "yes"), ("RestrictRealtime", "yes"), ("RestrictSUIDSGID", "yes")];
/// Address families permitted to the grepai template service.
pub(super) const GREPAI_RESTRICT_ADDRESS_FAMILIES: &str = "AF_UNIX AF_INET AF_INET6";

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
