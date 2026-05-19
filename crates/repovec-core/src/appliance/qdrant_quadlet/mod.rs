mod error;
mod parser;

#[cfg(test)]
mod api_key_tests;
#[cfg(test)]
mod provisioning_tests;
#[cfg(test)]
mod tests;
#[cfg(test)]
mod tests_proptest;
#[cfg(test)]
mod tests_proptest_strategies;

pub use error::QdrantQuadletError;
use parser::ParsedQuadlet;

const CHECKED_IN_QDRANT_QUADLET: &str =
    include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/../../packaging/systemd/qdrant.container"));

/// The repository path of the checked-in Qdrant Quadlet definition.
pub const CHECKED_IN_QDRANT_QUADLET_PATH: &str = "packaging/systemd/qdrant.container";

/// The installation path for the rootful system Quadlet.
pub const INSTALLED_QDRANT_QUADLET_PATH: &str = "/etc/containers/systemd/qdrant.container";

const QDRANT_API_KEY_SERVICE: &str = "repovec-qdrant-api-key.service";

const QDRANT_API_KEY_SECRET: &str = "repovec-qdrant-api-key";

const QDRANT_API_KEY_ENVIRONMENT_VARIABLE: &str = "QDRANT__SERVICE__API_KEY";

const UNIT_SECTION: &str = "Unit";
const CONTAINER_SECTION: &str = "Container";
/// The supported Qdrant OCI image reference for the appliance contract.
const REQUIRED_IMAGE: &str = "docker.io/qdrant/qdrant:v1";
/// The REST API port Qdrant exposes inside the container.
const REQUIRED_REST_PORT: u16 = 6333;
/// The gRPC API port Qdrant exposes inside the container.
const REQUIRED_GRPC_PORT: u16 = 6334;
/// The host path where the appliance stores Qdrant's persistent data.
const REQUIRED_STORAGE_SOURCE: &str = "/var/lib/repovec/qdrant-storage";

/// The REST API host binding for the appliance-managed Qdrant container.
///
/// Format: `IP:host_port:container_port`.
const REQUIRED_REST_BINDING: &str = "127.0.0.1:6333:6333";

/// The gRPC API host binding for the appliance-managed Qdrant container.
///
/// Format: `IP:host_port:container_port`.
const REQUIRED_GRPC_BINDING: &str = "127.0.0.1:6334:6334";
/// The in-container path where Qdrant stores persistent data.
const REQUIRED_STORAGE_TARGET: &str = "/qdrant/storage";
/// The Podman auto-update policy required for the appliance-managed service.
const REQUIRED_AUTO_UPDATE_POLICY: &str = "registry";
/// The `SELinux` relabel option required for the host storage mount.
const REQUIRED_SELINUX_OPTION: &str = "Z";
pub const fn checked_in_qdrant_quadlet() -> &'static str { CHECKED_IN_QDRANT_QUADLET }

/// Validates the repository's checked-in Qdrant Quadlet definition.
///
/// # Errors
///
/// Returns [`QdrantQuadletError`] when the checked-in asset no longer satisfies
/// the appliance contract.
///
/// # Examples
///
/// ```
/// use repovec_core::appliance::qdrant_quadlet::validate_checked_in_qdrant_quadlet;
///
/// validate_checked_in_qdrant_quadlet().expect("the checked-in qdrant quadlet remains valid");
/// ```
pub fn validate_checked_in_qdrant_quadlet() -> Result<(), QdrantQuadletError> {
    validate_qdrant_quadlet(checked_in_qdrant_quadlet())
}

/// Validates arbitrary Qdrant Quadlet contents against the appliance contract.
///
/// # Errors
///
/// Returns [`QdrantQuadletError`] describing the first contract violation.
///
/// # Examples
///
/// ```
/// use repovec_core::appliance::qdrant_quadlet::validate_qdrant_quadlet;
///
/// let contents = "\
/// [Unit]
/// Requires=repovec-qdrant-api-key.service
/// After=repovec-qdrant-api-key.service
///
/// [Container]
/// Image=docker.io/qdrant/qdrant:v1
/// AutoUpdate=registry
/// Secret=repovec-qdrant-api-key,type=env,target=QDRANT__SERVICE__API_KEY
/// PublishPort=127.0.0.1:6333:6333
/// PublishPort=127.0.0.1:6334:6334
/// Volume=/var/lib/repovec/qdrant-storage:/qdrant/storage:Z
/// ";
///
/// validate_qdrant_quadlet(contents).expect("the inline quadlet should satisfy the contract");
/// ```
pub fn validate_qdrant_quadlet(contents: &str) -> Result<(), QdrantQuadletError> {
    let parsed = ParsedQuadlet::parse(contents)?;

    validate_required_image(&parsed)?;
    validate_required_port(
        &parsed,
        REQUIRED_REST_PORT,
        REQUIRED_REST_BINDING,
        QdrantQuadletError::MissingRestPort,
    )?;
    validate_required_port(
        &parsed,
        REQUIRED_GRPC_PORT,
        REQUIRED_GRPC_BINDING,
        QdrantQuadletError::MissingGrpcPort,
    )?;
    validate_storage_mount(&parsed)?;
    validate_auto_update(&parsed)?;
    validate_api_key_provisioning_dependency(&parsed)?;
    validate_api_key_secret(&parsed)?;
    validate_no_inline_api_key_environment(&parsed)
}

fn validate_required_image(parsed: &ParsedQuadlet) -> Result<(), QdrantQuadletError> {
    let images = parsed.values(CONTAINER_SECTION, "Image");
    let image = match images {
        [] => return Err(QdrantQuadletError::MissingImage),
        [image] => image,
        duplicate_images => {
            return Err(QdrantQuadletError::UnexpectedImage { image: duplicate_images.join(",") });
        }
    };

    if !is_fully_qualified_and_pinned(image) {
        return Err(QdrantQuadletError::ImageNotFullyQualified { image: image.clone() });
    }

    if image != REQUIRED_IMAGE {
        return Err(QdrantQuadletError::UnexpectedImage { image: image.clone() });
    }

    Ok(())
}

fn is_fully_qualified_and_pinned(image: &str) -> bool {
    let Some((repository, tag)) = image.rsplit_once(':') else {
        return false;
    };
    let Some((registry, _path)) = repository.split_once('/') else {
        return false;
    };

    registry.contains('.') && !tag.is_empty() && tag != "latest"
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
    options.iter().any(|option| option.eq_ignore_ascii_case(REQUIRED_SELINUX_OPTION))
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

//! Validation helpers for the checked-in Qdrant Podman Quadlet asset.
//!
//! This module validates the repository's Quadlet asset against the complete
//! appliance contract. The contract deliberately includes both Qdrant domain
//! invariants and appliance platform bindings, because the Quadlet is the
//! integration point where those concerns meet.
//!
//! # Domain invariants
//!
//! These values express what the appliance expects from Qdrant itself:
//!
//! - the OCI image reference remains fully qualified and pinned to the supported
//!   Qdrant major line;
//! - persistent storage is mounted inside the container at `/qdrant/storage`;
//! - the REST API remains available on container port `6333`;
//! - the gRPC API remains available on container port `6334`.
//!
//! # Platform bindings
//!
//! These values express how the appliance makes those invariants safe and
//! operational on the host:
//!
//! - persistent data is sourced from `/var/lib/repovec/qdrant-storage`;
//! - Qdrant is published on `127.0.0.1` only;
//! - the storage mount carries the `SELinux` `:Z` relabel option;
//! - Podman auto-updates use the `registry` policy.
//!
//! Keeping the checks colocated makes the intentional boundary visible: Qdrant
//! defines the container contract, while the appliance platform defines the
//! host-side bindings that satisfy it.

mod api_key;
