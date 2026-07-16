//! Appliance-specific assets and validation helpers.
//!
//! This module groups appliance packaging and runtime validation surfaces that
//! compose the repovec appliance's Qdrant and systemd contracts at startup or
//! during tests:
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
//! - [`qdrant_liveness`] validates the live Qdrant gRPC endpoint using the
//!   stored appliance API key.
//!
//! - [`daemon_startup`] composes [`systemd_units`] and [`qdrant_liveness`]
//!   during daemon startup.
//!
//! The submodules compose the appliance surface: [`qdrant_quadlet`] covers the
//! Podman/Quadlet layer, [`systemd_units`] covers the systemd service
//! orchestration layer, and [`qdrant_liveness`] covers runtime readiness.

pub mod daemon_startup;
pub mod qdrant_liveness;
pub mod qdrant_quadlet;
pub mod systemd_units;
