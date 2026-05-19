mod error;
mod parser;
mod platform_bindings;
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


/// The in-container path where Qdrant stores persistent data.
const REQUIRED_STORAGE_TARGET: &str = "/qdrant/storage";
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
    validate_platform_bindings(&parsed)?;
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
//! Platform values express how the appliance makes those invariants safe and
//! operational on the host. The checks live in the `platform_bindings` adapter
//! module so host paths, loopback bindings, `SELinux` relabelling, and Podman
//! auto-update policy do not sit in the domain validation body:
//!
//! - persistent data is sourced from `/var/lib/repovec/qdrant-storage`;
//! - Qdrant is published on `127.0.0.1` only;
//! - the storage mount carries the `SELinux` `:Z` relabel option;
//! - Podman auto-updates use the `registry` policy.
//!
//! The public validator composes both sides of the contract: Qdrant defines the
//! container contract, while the appliance platform adapter defines the
//! host-side bindings that satisfy it.

mod api_key;
