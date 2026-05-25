//! Appliance platform binding checks for the Qdrant Quadlet contract.
//!
//! Qdrant defines the container-facing requirements. This adapter validates how
//! the repovec appliance binds those requirements to the host: loopback port
//! publishing, persistent storage location, `SELinux` relabelling, and Podman
//! auto-update policy.

use super::{
    CONTAINER_SECTION, QdrantQuadletError, QdrantQuadletObserver, REQUIRED_GRPC_PORT,
    REQUIRED_REST_PORT, REQUIRED_STORAGE_TARGET, parser::ParsedQuadlet,
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

struct RequiredPortBinding {
    container_port: u16,
    required_binding: &'static str,
    missing_error: QdrantQuadletError,
}

enum AutoUpdateValues<'a> {
    Empty,
    Single(&'a String),
    Duplicates(String),
}

/// Validate appliance-owned platform bindings for the Qdrant container.
pub(super) fn validate_platform_bindings(
    parsed: &ParsedQuadlet,
    observer: &dyn QdrantQuadletObserver,
) -> Result<(), QdrantQuadletError> {
    validate_required_port(
        parsed,
        RequiredPortBinding {
            container_port: REQUIRED_REST_PORT,
            required_binding: REQUIRED_REST_BINDING,
            missing_error: QdrantQuadletError::MissingRestPort,
        },
        observer,
    )?;
    validate_required_port(
        parsed,
        RequiredPortBinding {
            container_port: REQUIRED_GRPC_PORT,
            required_binding: REQUIRED_GRPC_BINDING,
            missing_error: QdrantQuadletError::MissingGrpcPort,
        },
        observer,
    )?;
    validate_storage_mount(parsed, observer)?;
    validate_auto_update(parsed, observer)
}

fn validate_required_port(
    parsed: &ParsedQuadlet,
    binding: RequiredPortBinding,
    observer: &dyn QdrantQuadletObserver,
) -> Result<(), QdrantQuadletError> {
    let publish_ports = parsed.values(CONTAINER_SECTION, "PublishPort");

    let matching_publish_ports = publish_ports
        .iter()
        .map(String::as_str)
        .filter(|port| published_container_port(port) == Some(binding.container_port))
        .collect::<Vec<_>>();

    if matching_publish_ports.is_empty() {
        observer.missing_publish_port(binding.container_port, binding.required_binding);
        return Err(binding.missing_error);
    }

    if let Some(publish_port) =
        matching_publish_ports.iter().find(|port| **port != binding.required_binding)
    {
        observer.publish_port_not_bound_to_loopback(
            binding.container_port,
            publish_port,
            binding.required_binding,
        );
        return Err(QdrantQuadletError::PortNotBoundToLoopback {
            port: binding.container_port,
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

fn validate_storage_mount(
    parsed: &ParsedQuadlet,
    observer: &dyn QdrantQuadletObserver,
) -> Result<(), QdrantQuadletError> {
    let volumes = parsed.values(CONTAINER_SECTION, "Volume");
    let mut last_candidate_error = None;

    for (volume, parts) in volumes.iter().filter_map(|volume| storage_mount_candidate(volume)) {
        let Some(source) = parts.first().copied() else {
            observer.missing_storage_mount_volume(
                volume,
                REQUIRED_STORAGE_SOURCE,
                REQUIRED_STORAGE_TARGET,
            );
            last_candidate_error = Some(QdrantQuadletError::MissingStorageMount);
            continue;
        };
        if source != REQUIRED_STORAGE_SOURCE {
            last_candidate_error =
                Some(QdrantQuadletError::IncorrectStorageSource { source: source.to_owned() });
            continue;
        }

        let Some(target) = parts.get(1).copied() else {
            observer.missing_storage_mount_volume(
                volume,
                REQUIRED_STORAGE_SOURCE,
                REQUIRED_STORAGE_TARGET,
            );
            last_candidate_error = Some(QdrantQuadletError::MissingStorageMount);
            continue;
        };
        if target != REQUIRED_STORAGE_TARGET {
            last_candidate_error =
                Some(QdrantQuadletError::IncorrectStorageTarget { target: target.to_owned() });
            continue;
        }

        if !parts.get(2..).is_some_and(has_required_selinux_relabel_option) {
            last_candidate_error =
                Some(QdrantQuadletError::MissingSelinuxRelabel { volume: volume.to_owned() });
            continue;
        }

        return Ok(());
    }

    let error = last_candidate_error.unwrap_or(QdrantQuadletError::MissingStorageMount);
    observe_storage_mount_error(observer, &error);
    Err(error)
}

fn observe_storage_mount_error(observer: &dyn QdrantQuadletObserver, error: &QdrantQuadletError) {
    match error {
        QdrantQuadletError::IncorrectStorageSource { source } => {
            observer.incorrect_storage_source(source, REQUIRED_STORAGE_SOURCE);
        }
        QdrantQuadletError::IncorrectStorageTarget { target } => {
            observer.incorrect_storage_target(target, REQUIRED_STORAGE_TARGET);
        }
        QdrantQuadletError::MissingSelinuxRelabel { volume } => {
            observer.missing_selinux_relabel(volume, REQUIRED_SELINUX_OPTION);
        }
        _ => observer.missing_storage_mount(REQUIRED_STORAGE_SOURCE, REQUIRED_STORAGE_TARGET),
    }
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
    let has_required_source =
        parts.first().is_some_and(|source| *source == REQUIRED_STORAGE_SOURCE);
    let has_required_target = parts.get(1).is_some_and(|target| *target == REQUIRED_STORAGE_TARGET);

    (has_required_source || has_required_target).then_some((volume, parts))
}

fn validate_auto_update(
    parsed: &ParsedQuadlet,
    observer: &dyn QdrantQuadletObserver,
) -> Result<(), QdrantQuadletError> {
    let auto_updates = parsed.values(CONTAINER_SECTION, "AutoUpdate");
    let auto_update = match classify_auto_update_values(auto_updates) {
        AutoUpdateValues::Empty => {
            observer.missing_auto_update(REQUIRED_AUTO_UPDATE_POLICY);
            return Err(QdrantQuadletError::MissingAutoUpdate);
        }
        AutoUpdateValues::Single(auto_update) => auto_update,
        AutoUpdateValues::Duplicates(duplicate_auto_update_values) => {
            observer
                .incorrect_auto_update(&duplicate_auto_update_values, REQUIRED_AUTO_UPDATE_POLICY);
            return Err(QdrantQuadletError::IncorrectAutoUpdate {
                auto_update: duplicate_auto_update_values,
            });
        }
    };

    if auto_update != REQUIRED_AUTO_UPDATE_POLICY {
        observer.incorrect_auto_update(auto_update, REQUIRED_AUTO_UPDATE_POLICY);
        return Err(QdrantQuadletError::IncorrectAutoUpdate { auto_update: auto_update.clone() });
    }

    Ok(())
}

fn classify_auto_update_values(auto_updates: &[String]) -> AutoUpdateValues<'_> {
    match auto_updates {
        [] => AutoUpdateValues::Empty,
        [auto_update] => AutoUpdateValues::Single(auto_update),
        duplicate_auto_updates => AutoUpdateValues::Duplicates(duplicate_auto_updates.join(",")),
    }
}

#[cfg(test)]
mod tests;
