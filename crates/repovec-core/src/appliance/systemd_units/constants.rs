//! Names, identity values, and hardening directives for the systemd unit contract.

pub(super) const TARGET_UNIT: &str = "repovec.target";
pub(super) const REPOVECD_UNIT: &str = "repovecd.service";
pub(super) const REPOVEC_MCPD_UNIT: &str = "repovec-mcpd.service";
pub(super) const REPOVEC_GREPAI_TEMPLATE_UNIT: &str = "repovec-grepai@.service";
pub(super) const UNIT_SECTION: &str = "Unit";
pub(super) const SERVICE_SECTION: &str = "Service";
pub(super) const INSTALL_SECTION: &str = "Install";
pub(super) const QDRANT_SERVICE: &str = "qdrant.service";
pub(super) const QDRANT_CONTAINER: &str = "qdrant.container";
pub(super) const QDRANT_CONTAINER_SERVICE: &str = "qdrant.container.service";
pub(super) const SERVICE_USER: &str = "repovec";
pub(super) const SERVICE_GROUP: &str = "repovec";
pub(super) const SERVICE_WORKING_DIRECTORY: &str = "/var/lib/repovec";
pub(super) const GREPAI_WORKING_DIRECTORY: &str = "/var/lib/repovec/worktrees/%I";
pub(super) const SERVICE_HOME_ENVIRONMENT: &str = "HOME=/var/lib/repovec";
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
pub(super) const GREPAI_BOOLEAN_HARDENING_DIRECTIVES: [(&str, &str); 3] =
    [("RestrictNamespaces", "yes"), ("RestrictRealtime", "yes"), ("RestrictSUIDSGID", "yes")];
pub(super) const GREPAI_RESTRICT_ADDRESS_FAMILIES: &str = "AF_UNIX AF_INET AF_INET6";
