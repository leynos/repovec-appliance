//! Validation helpers for the checked-in Qdrant Podman Quadlet asset.

mod error;
mod parser;

#[cfg(test)]
mod tests;

pub use error::QdrantQuadletError;
use parser::ParsedQuadlet;

const CHECKED_IN_QDRANT_QUADLET: &str =
    include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/../../packaging/systemd/qdrant.container"));

/// The repository path of the checked-in Qdrant Quadlet definition.
pub const CHECKED_IN_QDRANT_QUADLET_PATH: &str = "packaging/systemd/qdrant.container";

/// The installation path for the rootful system Quadlet.
pub const INSTALLED_QDRANT_QUADLET_PATH: &str = "/etc/containers/systemd/qdrant.container";

const CONTAINER_SECTION: &str = "Container";
const REQUIRED_IMAGE: &str = "docker.io/qdrant/qdrant:v1.17.1";
const REQUIRED_REST_PORT: &str = "127.0.0.1:6333:6333";
const REQUIRED_GRPC_PORT: &str = "127.0.0.1:6334:6334";
const REQUIRED_STORAGE_SOURCE: &str = "/var/lib/repovec/qdrant-storage";
const REQUIRED_STORAGE_TARGET: &str = "/qdrant/storage";
const REQUIRED_AUTO_UPDATE_POLICY: &str = "registry";

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
/// [Container]
/// Image=docker.io/qdrant/qdrant:v1.17.1
/// AutoUpdate=registry
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
    validate_required_port(&parsed, REQUIRED_REST_PORT, QdrantQuadletError::MissingRestPort)?;
    validate_required_port(&parsed, REQUIRED_GRPC_PORT, QdrantQuadletError::MissingGrpcPort)?;
    validate_storage_mount(&parsed)?;
    validate_auto_update(&parsed)
}

fn validate_required_image(parsed: &ParsedQuadlet) -> Result<(), QdrantQuadletError> {
    let Some(image) = parsed.values(CONTAINER_SECTION, "Image").first() else {
        return Err(QdrantQuadletError::MissingImage);
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
    expected_port: &str,
    missing_error: QdrantQuadletError,
) -> Result<(), QdrantQuadletError> {
    let publish_ports = parsed.values(CONTAINER_SECTION, "PublishPort");

    if publish_ports.iter().any(|port| port == expected_port) {
        return Ok(());
    }

    let container_port = expected_port
        .rsplit(':')
        .next()
        .and_then(|port| port.parse::<u16>().ok())
        .unwrap_or_default();

    if let Some(publish_port) =
        publish_ports.iter().find(|port| published_container_port(port) == Some(container_port))
    {
        return Err(QdrantQuadletError::PortNotBoundToLoopback {
            port: container_port,
            publish_port: publish_port.clone(),
        });
    }

    Err(missing_error)
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
    let Some(volume) = volumes.iter().find(|volume| volume.starts_with(REQUIRED_STORAGE_SOURCE))
    else {
        return Err(QdrantQuadletError::MissingStorageMount);
    };

    let parts: Vec<_> = volume.split(':').collect();
    if parts.len() < 2 {
        return Err(QdrantQuadletError::MissingStorageMount);
    }

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

    if !parts.get(2..).is_some_and(|options| options.contains(&"Z")) {
        return Err(QdrantQuadletError::MissingSelinuxRelabel { volume: volume.clone() });
    }

    Ok(())
}

fn validate_auto_update(parsed: &ParsedQuadlet) -> Result<(), QdrantQuadletError> {
    let Some(auto_update) = parsed.values(CONTAINER_SECTION, "AutoUpdate").first() else {
        return Err(QdrantQuadletError::MissingAutoUpdate);
    };

    if auto_update != REQUIRED_AUTO_UPDATE_POLICY {
        return Err(QdrantQuadletError::IncorrectAutoUpdate { auto_update: auto_update.clone() });
    }

    Ok(())
}
