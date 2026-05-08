//! Validation helpers for the checked-in Qdrant Podman Quadlet asset.

mod api_key;
mod error;
mod parser;

#[cfg(test)]
mod api_key_tests;
#[cfg(test)]
mod provisioning_tests;
#[cfg(test)]
mod tests;

use api_key::{
    validate_api_key_provisioning_dependency, validate_api_key_secret,
    validate_no_inline_api_key_environment,
};
pub use error::QdrantQuadletError;
use parser::ParsedQuadlet;
use tracing::{info, warn};

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
const REQUIRED_IMAGE: &str = "docker.io/qdrant/qdrant:v1";
const REQUIRED_REST_PORT: &str = "127.0.0.1:6333:6333";
const REQUIRED_GRPC_PORT: &str = "127.0.0.1:6334:6334";
const REQUIRED_STORAGE_SOURCE: &str = "/var/lib/repovec/qdrant-storage";
const REQUIRED_STORAGE_TARGET: &str = "/qdrant/storage";
const REQUIRED_AUTO_UPDATE_POLICY: &str = "registry";
const LOG_TARGET: &str = "repovec_core::qdrant_quadlet";

/// Returns the repository's checked-in Qdrant Quadlet source.
///
/// # Examples
///
/// ```
/// use repovec_core::appliance::qdrant_quadlet::checked_in_qdrant_quadlet;
///
/// assert!(checked_in_qdrant_quadlet().contains("[Container]"));
/// ```
#[must_use]
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
    info!(
        target: LOG_TARGET,
        path = CHECKED_IN_QDRANT_QUADLET_PATH,
        "validating checked-in qdrant quadlet"
    );
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
    info!(target: LOG_TARGET, "validating qdrant quadlet contract");

    let parsed = ParsedQuadlet::parse(contents)?;

    validate_required_image(&parsed)?;
    validate_required_port(&parsed, REQUIRED_REST_PORT, QdrantQuadletError::MissingRestPort)?;
    validate_required_port(&parsed, REQUIRED_GRPC_PORT, QdrantQuadletError::MissingGrpcPort)?;
    validate_storage_mount(&parsed)?;
    validate_auto_update(&parsed)?;
    validate_api_key_provisioning_dependency(&parsed)?;
    validate_api_key_secret(&parsed)?;
    validate_no_inline_api_key_environment(&parsed)?;

    info!(target: LOG_TARGET, "qdrant quadlet contract validation succeeded");

    Ok(())
}

fn validate_required_image(parsed: &ParsedQuadlet) -> Result<(), QdrantQuadletError> {
    let images = parsed.values(CONTAINER_SECTION, "Image");
    let image = match images {
        [] => {
            warn!(
                target: LOG_TARGET,
                expected_image = REQUIRED_IMAGE,
                "qdrant quadlet validation failed: missing image"
            );
            return Err(QdrantQuadletError::MissingImage);
        }
        [image] => image,
        duplicate_images => {
            let duplicate_image_values = duplicate_images.join(",");
            warn!(
                target: LOG_TARGET,
                image = duplicate_image_values,
                expected_image = REQUIRED_IMAGE,
                "qdrant quadlet validation failed: unexpected image"
            );
            return Err(QdrantQuadletError::UnexpectedImage { image: duplicate_image_values });
        }
    };

    if !is_fully_qualified_and_pinned(image) {
        warn!(
            target: LOG_TARGET,
            image,
            expected_image = REQUIRED_IMAGE,
            "qdrant quadlet validation failed: image is not fully qualified and pinned"
        );
        return Err(QdrantQuadletError::ImageNotFullyQualified { image: image.clone() });
    }

    if image != REQUIRED_IMAGE {
        warn!(
            target: LOG_TARGET,
            image,
            expected_image = REQUIRED_IMAGE,
            "qdrant quadlet validation failed: unexpected image"
        );
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
    expected_port: &str,
    missing_error: QdrantQuadletError,
) -> Result<(), QdrantQuadletError> {
    let publish_ports = parsed.values(CONTAINER_SECTION, "PublishPort");
    let container_port = expected_port
        .rsplit(':')
        .next()
        .and_then(|port| port.parse::<u16>().ok())
        .unwrap_or_default();

    let matching_publish_ports = publish_ports
        .iter()
        .map(String::as_str)
        .filter(|port| published_container_port(port) == Some(container_port))
        .collect::<Vec<_>>();

    if matching_publish_ports.is_empty() {
        warn!(
            target: LOG_TARGET,
            port = container_port,
            expected_publish_port = expected_port,
            "qdrant quadlet validation failed: missing publish port"
        );
        return Err(missing_error);
    }

    if let Some(publish_port) = matching_publish_ports.iter().find(|port| **port != expected_port) {
        warn!(
            target: LOG_TARGET,
            port = container_port,
            publish_port,
            expected_publish_port = expected_port,
            "qdrant quadlet validation failed: publish port is not bound to loopback"
        );
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
        warn!(
            target: LOG_TARGET,
            expected_source = REQUIRED_STORAGE_SOURCE,
            expected_target = REQUIRED_STORAGE_TARGET,
            "qdrant quadlet validation failed: missing storage mount"
        );
        return Err(QdrantQuadletError::MissingStorageMount);
    };

    let Some(source) = parts.first().copied() else {
        warn!(
            target: LOG_TARGET,
            volume,
            expected_source = REQUIRED_STORAGE_SOURCE,
            expected_target = REQUIRED_STORAGE_TARGET,
            "qdrant quadlet validation failed: missing storage mount"
        );
        return Err(QdrantQuadletError::MissingStorageMount);
    };
    if source != REQUIRED_STORAGE_SOURCE {
        warn!(
            target: LOG_TARGET,
            source,
            expected_source = REQUIRED_STORAGE_SOURCE,
            "qdrant quadlet validation failed: incorrect storage source"
        );
        return Err(QdrantQuadletError::IncorrectStorageSource { source: source.to_owned() });
    }

    let Some(target) = parts.get(1).copied() else {
        warn!(
            target: LOG_TARGET,
            volume,
            expected_source = REQUIRED_STORAGE_SOURCE,
            expected_target = REQUIRED_STORAGE_TARGET,
            "qdrant quadlet validation failed: missing storage mount"
        );
        return Err(QdrantQuadletError::MissingStorageMount);
    };
    if target != REQUIRED_STORAGE_TARGET {
        warn!(
            target: LOG_TARGET,
            target,
            expected_target = REQUIRED_STORAGE_TARGET,
            "qdrant quadlet validation failed: incorrect storage target"
        );
        return Err(QdrantQuadletError::IncorrectStorageTarget { target: target.to_owned() });
    }

    if !parts.get(2..).is_some_and(|options| options.contains(&"Z")) {
        warn!(
            target: LOG_TARGET,
            volume,
            expected_selinux_relabel = "Z",
            "qdrant quadlet validation failed: missing selinux relabel"
        );
        return Err(QdrantQuadletError::MissingSelinuxRelabel { volume: volume.to_owned() });
    }

    Ok(())
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
        [] => {
            warn!(
                target: LOG_TARGET,
                expected_auto_update = REQUIRED_AUTO_UPDATE_POLICY,
                "qdrant quadlet validation failed: missing auto-update policy"
            );
            return Err(QdrantQuadletError::MissingAutoUpdate);
        }
        [auto_update] => auto_update,
        duplicate_auto_updates => {
            let duplicate_auto_update_values = duplicate_auto_updates.join(",");
            warn!(
                target: LOG_TARGET,
                auto_update = duplicate_auto_update_values,
                expected_auto_update = REQUIRED_AUTO_UPDATE_POLICY,
                "qdrant quadlet validation failed: incorrect auto-update policy"
            );
            return Err(QdrantQuadletError::IncorrectAutoUpdate {
                auto_update: duplicate_auto_update_values,
            });
        }
    };

    if auto_update != REQUIRED_AUTO_UPDATE_POLICY {
        warn!(
            target: LOG_TARGET,
            auto_update,
            expected_auto_update = REQUIRED_AUTO_UPDATE_POLICY,
            "qdrant quadlet validation failed: incorrect auto-update policy"
        );
        return Err(QdrantQuadletError::IncorrectAutoUpdate { auto_update: auto_update.clone() });
    }

    Ok(())
}
