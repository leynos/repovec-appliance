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

/// Expected host binding and missing-port error for one container port.
struct RequiredPortBinding {
    /// The port exposed inside the Qdrant container.
    container_port: u16,
    /// The exact loopback host-to-container mapping required by the appliance.
    required_binding: &'static str,
    /// The domain error reported when no matching container port is published.
    missing_error: QdrantQuadletError,
}

/// Classification of the container's `AutoUpdate` directives.
enum AutoUpdateValues<'a> {
    /// No directive is present.
    Empty,
    /// Exactly one directive is present.
    Single(&'a String),
    /// Multiple directives were joined for a diagnostic.
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

/// Verifies that a container port is published through its required loopback binding.
///
/// The binding supplies the target port, expected text, and missing-value
/// error. For example, `127.0.0.1:6333:6333` satisfies the REST binding.
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

/// Extracts the container port from a three-part host publication value.
///
/// `127.0.0.1:6333:6333` returns `Some(6333)`; malformed or non-numeric
/// values return `None`.
fn published_container_port(publish_port: &str) -> Option<u16> {
    let parts: Vec<_> = publish_port.split(':').collect();
    if parts.len() != 3 {
        return None;
    }

    parts.get(2)?.parse::<u16>().ok()
}

/// Verifies the required persistent storage source, target, and `SELinux` option.
///
/// The parsed volume entries are checked in order, and the observer receives
/// the final contract violation. A valid example is
/// `/var/lib/repovec/qdrant-storage:/qdrant/storage:Z`.
fn validate_storage_mount(
    parsed: &ParsedQuadlet,
    observer: &dyn QdrantQuadletObserver,
) -> Result<(), QdrantQuadletError> {
    let volumes = parsed.values(CONTAINER_SECTION, "Volume");
    let mut last_candidate_error = None;
    let mut missing_storage_mount_volume = None;

    for (volume, parts) in volumes.iter().filter_map(|volume| storage_mount_candidate(volume)) {
        let Some(source) = parts.first().copied() else {
            missing_storage_mount_volume = Some(volume);
            last_candidate_error = Some(QdrantQuadletError::MissingStorageMount);
            continue;
        };
        if source != REQUIRED_STORAGE_SOURCE {
            last_candidate_error =
                Some(QdrantQuadletError::IncorrectStorageSource { source: source.to_owned() });
            continue;
        }

        let Some(target) = parts.get(1).copied() else {
            missing_storage_mount_volume = Some(volume);
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
    observe_storage_mount_error(observer, &error, missing_storage_mount_volume);
    Err(error)
}

/// Sends the specific storage validation error to the observer.
///
/// The error and any incomplete candidate volume determine which observer
/// event is emitted; a missing candidate reports the general missing-mount event.
fn observe_storage_mount_error(
    observer: &dyn QdrantQuadletObserver,
    error: &QdrantQuadletError,
    missing_storage_mount_volume: Option<&str>,
) {
    match (error, missing_storage_mount_volume) {
        (QdrantQuadletError::IncorrectStorageSource { source }, _) => {
            observer.incorrect_storage_source(source, REQUIRED_STORAGE_SOURCE);
        }
        (QdrantQuadletError::IncorrectStorageTarget { target }, _) => {
            observer.incorrect_storage_target(target, REQUIRED_STORAGE_TARGET);
        }
        (QdrantQuadletError::MissingSelinuxRelabel { volume }, _) => {
            observer.missing_selinux_relabel(volume, REQUIRED_SELINUX_OPTION);
        }
        (QdrantQuadletError::MissingStorageMount, Some(volume)) => {
            observer.missing_storage_mount_volume(
                volume,
                REQUIRED_STORAGE_SOURCE,
                REQUIRED_STORAGE_TARGET,
            );
        }
        _ => observer.missing_storage_mount(REQUIRED_STORAGE_SOURCE, REQUIRED_STORAGE_TARGET),
    }
}

/// Reports whether any comma-separated volume option contains the required relabel flag.
///
/// The groups are trimmed before comparison, so `ro,Z` satisfies the required
/// `Z` option while `z` does not.
fn has_required_selinux_relabel_option(options: &[&str]) -> bool {
    options
        .iter()
        .flat_map(|group| group.split(','))
        .map(str::trim)
        .any(|option| option == REQUIRED_SELINUX_OPTION)
}

/// Splits a volume entry when its source or target resembles the required mount.
///
/// A matching source or target returns the original entry with colon-separated
/// parts; an unrelated volume returns `None`.
fn storage_mount_candidate(volume: &str) -> Option<(&str, Vec<&str>)> {
    let parts = volume.split(':').collect::<Vec<_>>();
    let has_required_source =
        parts.first().is_some_and(|source| *source == REQUIRED_STORAGE_SOURCE);
    let has_required_target = parts.get(1).is_some_and(|target| *target == REQUIRED_STORAGE_TARGET);

    (has_required_source || has_required_target).then_some((volume, parts))
}

/// Requires exactly one `AutoUpdate` directive with the appliance's registry policy.
///
/// Missing, repeated, or unexpected values are reported through the observer
/// and returned as domain errors. `AutoUpdate=registry` passes.
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

/// Classifies auto-update values and joins duplicates for an error message.
///
/// An empty slice yields `Empty`, one value yields `Single`, and multiple
/// values yield `Duplicates`.
fn classify_auto_update_values(auto_updates: &[String]) -> AutoUpdateValues<'_> {
    match auto_updates {
        [] => AutoUpdateValues::Empty,
        [auto_update] => AutoUpdateValues::Single(auto_update),
        duplicate_auto_updates => AutoUpdateValues::Duplicates(duplicate_auto_updates.join(",")),
    }
}

#[cfg(test)]
mod tests;
