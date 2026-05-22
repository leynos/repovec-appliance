//! Appliance platform binding checks for the Qdrant Quadlet contract.
//!
//! Qdrant defines the container-facing requirements. This adapter validates how
//! the repovec appliance binds those requirements to the host: loopback port
//! publishing, persistent storage location, `SELinux` relabelling, and Podman
//! auto-update policy.

use super::{
    CONTAINER_SECTION, QdrantQuadletError, REQUIRED_GRPC_PORT, REQUIRED_REST_PORT,
    REQUIRED_STORAGE_TARGET, parser::ParsedQuadlet,
};

/// The host path where the appliance stores Qdrant's persistent data.
pub(super) const REQUIRED_STORAGE_SOURCE: &str = "/var/lib/repovec/qdrant-storage";
/// The REST API host binding for the appliance-managed Qdrant container.
///
/// Format: `IP:host_port:container_port`.
const REQUIRED_REST_BINDING: &str = "127.0.0.1:6333:6333";
/// The gRPC API host binding for the appliance-managed Qdrant container.
///
/// Format: `IP:host_port:container_port`.
const REQUIRED_GRPC_BINDING: &str = "127.0.0.1:6334:6334";
/// The Podman auto-update policy required for the appliance-managed service.
pub(super) const REQUIRED_AUTO_UPDATE_POLICY: &str = "registry";
/// The `SELinux` relabel option required for the host storage mount.
pub(super) const REQUIRED_SELINUX_OPTION: &str = "Z";

/// Validate appliance-owned platform bindings for the Qdrant container.
pub(super) fn validate_platform_bindings(parsed: &ParsedQuadlet) -> Result<(), QdrantQuadletError> {
    validate_required_port(
        parsed,
        REQUIRED_REST_PORT,
        REQUIRED_REST_BINDING,
        QdrantQuadletError::MissingRestPort,
    )?;
    validate_required_port(
        parsed,
        REQUIRED_GRPC_PORT,
        REQUIRED_GRPC_BINDING,
        QdrantQuadletError::MissingGrpcPort,
    )?;
    validate_storage_mount(parsed)?;
    validate_auto_update(parsed)
}

fn validate_required_port(
    parsed: &ParsedQuadlet,
    container_port: u16,
    required_binding: &str,
    missing_error: QdrantQuadletError,
) -> Result<(), QdrantQuadletError> {
    let publish_ports = parsed.values(CONTAINER_SECTION, "PublishPort");

    let matching_publish_ports = publish_ports
        .iter()
        .map(String::as_str)
        .filter(|port| published_container_port(port) == Some(container_port))
        .collect::<Vec<_>>();

    if matching_publish_ports.is_empty() {
        return Err(missing_error);
    }

    if let Some(publish_port) =
        matching_publish_ports.iter().find(|port| **port != required_binding)
    {
        return Err(QdrantQuadletError::PortNotBoundToLoopback {
            port: container_port,
            publish_port: (*publish_port).to_owned(),
        });
    }

    Ok(())
}

fn published_container_port(publish_port: &str) -> Option<u16> {
    let parts: Vec<_> = publish_port.split(':').collect();
    if parts.len() != 3 {
        return None;
    }

    parts.get(2)?.parse::<u16>().ok()
}

fn validate_storage_mount(parsed: &ParsedQuadlet) -> Result<(), QdrantQuadletError> {
    let volumes = parsed.values(CONTAINER_SECTION, "Volume");
    let Some((volume, parts)) = volumes.iter().find_map(|volume| storage_mount_candidate(volume))
    else {
        return Err(QdrantQuadletError::MissingStorageMount);
    };

    let Some(source) = parts.first().copied() else {
        return Err(QdrantQuadletError::MissingStorageMount);
    };
    if source != REQUIRED_STORAGE_SOURCE {
        return Err(QdrantQuadletError::IncorrectStorageSource { source: source.to_owned() });
    }

    let Some(target) = parts.get(1).copied() else {
        return Err(QdrantQuadletError::MissingStorageMount);
    };
    if target != REQUIRED_STORAGE_TARGET {
        return Err(QdrantQuadletError::IncorrectStorageTarget { target: target.to_owned() });
    }

    if !parts.get(2..).is_some_and(has_required_selinux_relabel_option) {
        return Err(QdrantQuadletError::MissingSelinuxRelabel { volume: volume.to_owned() });
    }

    Ok(())
}

fn has_required_selinux_relabel_option(options: &[&str]) -> bool {
    options
        .iter()
        .flat_map(|group| group.split(','))
        .map(str::trim)
        .any(|option| option.eq_ignore_ascii_case(REQUIRED_SELINUX_OPTION))
}

fn storage_mount_candidate(volume: &str) -> Option<(&str, Vec<&str>)> {
    let parts = volume.split(':').collect::<Vec<_>>();
    if parts.len() < 2 {
        return None;
    }

    let has_required_source =
        parts.first().is_some_and(|source| *source == REQUIRED_STORAGE_SOURCE);
    let has_required_target = parts.get(1).is_some_and(|target| *target == REQUIRED_STORAGE_TARGET);

    (has_required_source || has_required_target).then_some((volume, parts))
}

fn validate_auto_update(parsed: &ParsedQuadlet) -> Result<(), QdrantQuadletError> {
    let auto_updates = parsed.values(CONTAINER_SECTION, "AutoUpdate");
    let auto_update = match auto_updates {
        [] => return Err(QdrantQuadletError::MissingAutoUpdate),
        [auto_update] => auto_update,
        duplicate_auto_updates => {
            return Err(QdrantQuadletError::IncorrectAutoUpdate {
                auto_update: duplicate_auto_updates.join(","),
            });
        }
    };

    if auto_update != REQUIRED_AUTO_UPDATE_POLICY {
        return Err(QdrantQuadletError::IncorrectAutoUpdate { auto_update: auto_update.clone() });
    }

    Ok(())
}
