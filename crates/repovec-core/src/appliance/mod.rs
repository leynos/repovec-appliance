//! Appliance-specific assets and validation helpers.
//!
//! This module groups the two static validation surfaces that validate the
//! repovec appliance's packaging contract against checked-in assets at startup
//! or during tests:
//!
//! - [`qdrant_quadlet`] validates the checked-in Qdrant Quadlet container
//!   definition (`packaging/systemd/qdrant.container`), enforcing the pinned
//!   image reference, loopback-only REST and gRPC port bindings, storage mount,
//!   Podman auto-update policy, and API-key provisioning contract.
//!
//! - [`systemd_units`] validates the checked-in repovec systemd unit files
//!   (`repovec.target`, `repovecd.service`, and `repovec-mcpd.service`),
//!   enforcing dependency ordering, install targets, service identity, and
//!   `ExecStart` paths against the appliance service-layout contract.
//!
//! The two submodules are independent: [`qdrant_quadlet`] covers the
//! Podman/Quadlet layer, while [`systemd_units`] covers the systemd service
//! orchestration layer. Daemon binaries enforce the runtime startup contract by
//! calling [`systemd_units::validate_checked_in_systemd_units`] before doing
//! other work and treating any [`systemd_units::SystemdUnitError`] as a fatal
//! error.

pub mod qdrant_quadlet;
pub mod systemd_units;
