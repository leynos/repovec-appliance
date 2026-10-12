//! Names, identity values, and hardening directives for the systemd unit contract.

/// Name used to identify the appliance target in validation errors.
pub(super) const TARGET_UNIT: &str = "repovec.target";
/// Name of the repovecd service unit.
pub(super) const REPOVECD_UNIT: &str = "repovecd.service";
/// Name of the repovec MCP daemon service unit.
pub(super) const REPOVEC_MCPD_UNIT: &str = "repovec-mcpd.service";
/// Name of the templated grepai service unit.
pub(super) const REPOVEC_GREPAI_TEMPLATE_UNIT: &str = "repovec-grepai@.service";
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
