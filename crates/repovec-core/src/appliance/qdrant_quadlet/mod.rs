//! Public API and validation pipeline for the Qdrant Podman Quadlet contract.
//!
//! This module has two responsibilities. It exposes the public validation
//! surface, including [`QdrantQuadletError`],
//! [`validate_checked_in_qdrant_quadlet`], [`validate_qdrant_quadlet`],
//! [`QdrantQuadletObserver`], [`TracingQdrantQuadletObserver`],
//! [`CHECKED_IN_QDRANT_QUADLET_PATH`], and [`INSTALLED_QDRANT_QUADLET_PATH`].
//! It also orchestrates the full validation pipeline over a parsed Quadlet.
//!
//! [`validate_checked_in_qdrant_quadlet`] verifies the embedded packaging asset
//! during startup checks. [`validate_qdrant_quadlet`] validates
//! operator-supplied contract changes at runtime using the same appliance
//! policy.
//!
//! `LOG_TARGET` provides one tracing target for the module family. The tracing
//! observer uses it so operators can filter Qdrant Quadlet diagnostics with
//! `RUST_LOG=repovec_core::qdrant_quadlet=info`.
//!
//! The `parser` submodule produces a `ParsedQuadlet`. Structural validators in
//! this module consume it first, `platform_bindings` validates host-side
//! appliance bindings, then `api_key` validators consume the same parsed
//! representation for API-key-specific checks.

mod api_key;
mod error;
mod observer;
mod parser;
mod platform_bindings;

#[cfg(test)]
mod api_key_tests;
#[cfg(test)]
mod log_tests;
#[cfg(test)]
mod provisioning_tests;
#[cfg(test)]
mod tests;
#[cfg(test)]
mod tests_proptest;
#[cfg(test)]
mod tests_proptest_strategies;

use api_key::{
    validate_api_key_provisioning_dependency, validate_api_key_secret,
    validate_no_inline_api_key_environment,
};
pub use error::QdrantQuadletError;
pub use observer::{QdrantQuadletObserver, TracingQdrantQuadletObserver};
use parser::ParsedQuadlet;
use platform_bindings::validate_platform_bindings;

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

// Domain invariants.
/// The supported Qdrant OCI image reference for the appliance contract.
const REQUIRED_IMAGE: &str = "docker.io/qdrant/qdrant:v1";
/// The REST API port Qdrant exposes inside the container.
const REQUIRED_REST_PORT: u16 = 6333;
/// The gRPC API port Qdrant exposes inside the container.
const REQUIRED_GRPC_PORT: u16 = 6334;
/// The in-container path where Qdrant stores persistent data.
const REQUIRED_STORAGE_TARGET: &str = "/qdrant/storage";
pub(super) const LOG_TARGET: &str = "repovec_core::qdrant_quadlet";

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
/// # Parameters
///
/// - `observer`: Receives validation lifecycle and contract-violation events.
///   Pass [`TracingQdrantQuadletObserver`] to emit structured `tracing` events,
///   or `&()` when validation should use the no-op observer sink.
///
/// # Examples
///
/// ```rust,no_run
/// use repovec_core::appliance::qdrant_quadlet::{
///     TracingQdrantQuadletObserver, validate_checked_in_qdrant_quadlet,
/// };
///
/// validate_checked_in_qdrant_quadlet(&TracingQdrantQuadletObserver)
///     .expect("the checked-in qdrant quadlet remains valid");
/// ```
pub fn validate_checked_in_qdrant_quadlet(
    observer: &dyn QdrantQuadletObserver,
) -> Result<(), QdrantQuadletError> {
    observer.validating_checked_in_qdrant_quadlet(CHECKED_IN_QDRANT_QUADLET_PATH);
    validate_qdrant_quadlet(checked_in_qdrant_quadlet(), observer)
}

/// Validates arbitrary Qdrant Quadlet contents against the appliance contract.
///
/// # Errors
///
/// Returns [`QdrantQuadletError`] describing the first contract violation.
///
/// # Parameters
///
/// - `contents`: The Quadlet source to parse and validate.
/// - `observer`: Receives validation lifecycle and contract-violation events.
///   Pass [`TracingQdrantQuadletObserver`] to emit structured `tracing` events,
///   or `&()` when validation should use the no-op observer sink.
///
/// # Examples
///
/// ```rust,no_run
/// use repovec_core::appliance::qdrant_quadlet::{
///     TracingQdrantQuadletObserver, validate_qdrant_quadlet,
/// };
///
/// let contents = concat!(
///     "[Unit]\n",
///     "Requires=repovec-qdrant-api-key.service\n",
///     "After=repovec-qdrant-api-key.service\n",
///     "\n",
///     "[Container]\n",
///     "Image=docker.io/qdrant/qdrant:v1\n",
///     "AutoUpdate=registry\n",
///     "Secret=repovec-qdrant-api-key,type=env,target=QDRANT__SERVICE__API_KEY\n",
///     "PublishPort=127.0.0.1:6333:6333\n",
///     "PublishPort=127.0.0.1:6334:6334\n",
///     "Volume=/var/lib/repovec/qdrant-storage:/qdrant/storage:Z\n",
/// );
///
/// validate_qdrant_quadlet(contents, &TracingQdrantQuadletObserver)
///     .expect("the inline quadlet should satisfy the contract");
/// ```
pub fn validate_qdrant_quadlet(
    contents: &str,
    observer: &dyn QdrantQuadletObserver,
) -> Result<(), QdrantQuadletError> {
    observer.validating_qdrant_quadlet_contract();

    let parsed = ParsedQuadlet::parse(contents, observer)?;

    validate_required_image(&parsed, observer)?;
    validate_platform_bindings(&parsed, observer)?;
    validate_api_key_provisioning_dependency(&parsed, observer)?;
    validate_api_key_secret(&parsed, observer)?;
    validate_no_inline_api_key_environment(&parsed, observer)?;

    observer.qdrant_quadlet_contract_validation_succeeded();

    Ok(())
}

fn validate_required_image(
    parsed: &ParsedQuadlet,
    observer: &dyn QdrantQuadletObserver,
) -> Result<(), QdrantQuadletError> {
    let images = parsed.values(CONTAINER_SECTION, "Image");
    let image = match classify_directive_values(images) {
        DirectiveValues::Missing => {
            observer.missing_image(REQUIRED_IMAGE);
            return Err(QdrantQuadletError::MissingImage);
        }
        DirectiveValues::Single(image) => image,
        DirectiveValues::Duplicate(duplicate_image_values) => {
            observer.unexpected_image(&duplicate_image_values, REQUIRED_IMAGE);
            return Err(QdrantQuadletError::UnexpectedImage { image: duplicate_image_values });
        }
    };

    if !is_fully_qualified_and_pinned(image) {
        observer.image_not_fully_qualified(image, REQUIRED_IMAGE);
        return Err(QdrantQuadletError::ImageNotFullyQualified { image: image.to_owned() });
    }

    if image != REQUIRED_IMAGE {
        observer.unexpected_image(image, REQUIRED_IMAGE);
        return Err(QdrantQuadletError::UnexpectedImage { image: image.to_owned() });
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

enum DirectiveValues<'a> {
    Missing,
    Single(&'a str),
    Duplicate(String),
}

fn classify_directive_values(values: &[String]) -> DirectiveValues<'_> {
    match values {
        [] => DirectiveValues::Missing,
        [value] => DirectiveValues::Single(value),
        duplicate_values => DirectiveValues::Duplicate(duplicate_values.join(",")),
    }
}
