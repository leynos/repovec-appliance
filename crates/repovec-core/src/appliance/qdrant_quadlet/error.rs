//! Semantic validation errors for the Qdrant Quadlet contract.
//!
//! `QdrantQuadletError` is returned exclusively by `validate_qdrant_quadlet` in
//! `mod.rs`. It is the validator's public observability surface: callers
//! inspect the returned typed error, not tracing, logging, or metrics emitted
//! during validation. Its `Display` output is snapshot-tested in
//! `api_key_tests.rs` for API-key-specific failures and in the sibling
//! `tests.rs` module for the broader Quadlet contract.

use std::{error::Error, fmt};

use super::{
    QDRANT_API_KEY_SERVICE, REQUIRED_GRPC_PORT, REQUIRED_IMAGE, REQUIRED_REST_PORT,
    REQUIRED_STORAGE_TARGET,
    platform_bindings::{
        REQUIRED_AUTO_UPDATE_POLICY, REQUIRED_SELINUX_OPTION, REQUIRED_STORAGE_SOURCE,
    },
};

/// Contract failures for the Qdrant Quadlet.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum QdrantQuadletError {
    /// The Quadlet file could not be parsed as a small subset of INI syntax.
    InvalidLine {
        /// The 1-indexed source line number.
        line_number: usize,
        /// The invalid line contents after trimming and redaction.
        line: String,
    },
    /// A key-value pair appeared before any section header.
    PropertyBeforeSection {
        /// The 1-indexed source line number.
        line_number: usize,
        /// The misplaced property line after trimming and redaction.
        line: String,
    },
    /// No `Image=` entry exists in the `[Container]` section.
    MissingImage,
    /// The image reference is not fully qualified and explicitly tagged.
    ImageNotFullyQualified {
        /// The invalid image reference.
        image: String,
    },
    /// The image reference does not match the project contract.
    UnexpectedImage {
        /// The unexpected image reference.
        image: String,
    },
    /// The REST port mapping is missing.
    MissingRestPort,
    /// The gRPC port mapping is missing.
    MissingGrpcPort,
    /// A required port is not bound to the loopback address.
    PortNotBoundToLoopback {
        /// The required Qdrant container port.
        port: u16,
        /// The offending published port mapping.
        publish_port: String,
    },
    /// The persistent storage mount is absent.
    MissingStorageMount,
    /// The persistent storage source path is wrong.
    IncorrectStorageSource {
        /// The unexpected host-side storage path.
        source: String,
    },
    /// The persistent storage target path is wrong.
    IncorrectStorageTarget {
        /// The unexpected in-container storage path.
        target: String,
    },
    /// The storage mount is missing the `SELinux` relabel option.
    MissingSelinuxRelabel {
        /// The offending volume mapping.
        volume: String,
    },
    /// The Podman auto-update policy is absent.
    MissingAutoUpdate,
    /// The Podman auto-update policy is not `registry`.
    IncorrectAutoUpdate {
        /// The unexpected auto-update policy value.
        auto_update: String,
    },
    /// The Quadlet does not depend on the API-key provisioning unit.
    MissingApiKeyProvisioningDependency {
        /// The missing systemd dependency directive.
        directive: &'static str,
    },
    /// The Quadlet depends on the wrong API-key provisioning unit.
    IncorrectApiKeyProvisioningDependency {
        /// The systemd dependency directive with the wrong value.
        directive: &'static str,
        /// The unexpected dependency value.
        dependency: String,
    },
    /// The Quadlet does not depend on a required provisioning unit other than
    /// the API-key oneshot.
    MissingUnitDependency {
        /// The missing systemd dependency directive.
        directive: &'static str,
        /// The systemd unit the directive must reference.
        expected: &'static str,
    },
    /// The Quadlet depends on the wrong provisioning unit other than the
    /// API-key oneshot.
    IncorrectUnitDependency {
        /// The systemd dependency directive with the wrong value.
        directive: &'static str,
        /// The systemd unit the directive must reference.
        expected: &'static str,
        /// The unexpected dependency value.
        actual: String,
    },
    /// The API-key Podman secret is not present in the container definition.
    MissingApiKeySecret,
    /// The API-key Podman secret is present but wired to the wrong secret or target.
    IncorrectApiKeySecret {
        /// The unexpected secret value.
        secret: String,
    },
    /// The Quadlet exposes the Qdrant API key through an inline environment entry.
    InlineApiKeyEnvironmentDisallowed {
        /// The redacted offending environment value.
        environment: String,
    },
}

impl fmt::Display for QdrantQuadletError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidLine { line_number, line } => {
                write!(f, "invalid quadlet line {line_number}: {line}")
            }
            Self::PropertyBeforeSection { line_number, line } => {
                write!(f, "quadlet property before section on line {line_number}: {line}")
            }
            Self::MissingImage => f.write_str("missing Image= entry in [Container]"),
            Self::ImageNotFullyQualified { image } => {
                write!(f, "image reference must be fully qualified and tagged: {image}")
            }
            Self::UnexpectedImage { image } => {
                write!(f, "image reference must remain {REQUIRED_IMAGE}: {image}")
            }
            Self::MissingRestPort => {
                write!(f, "missing PublishPort={REQUIRED_REST_PORT} in [Container]")
            }
            Self::MissingGrpcPort => {
                write!(f, "missing PublishPort={REQUIRED_GRPC_PORT} in [Container]")
            }
            Self::PortNotBoundToLoopback { port, publish_port } => {
                write!(f, "port {port} must be published on 127.0.0.1 only: {publish_port}")
            }
            Self::MissingStorageMount => f.write_str("missing persistent Qdrant storage mount"),
            Self::IncorrectStorageSource { source } => {
                write!(f, "storage source must be {REQUIRED_STORAGE_SOURCE}: {source}")
            }
            Self::IncorrectStorageTarget { target } => {
                write!(f, "storage target must be {REQUIRED_STORAGE_TARGET}: {target}")
            }
            Self::MissingSelinuxRelabel { volume } => {
                write!(
                    f,
                    "storage mount must include SELinux relabel :{REQUIRED_SELINUX_OPTION}: \
                     {volume}"
                )
            }
            Self::MissingAutoUpdate => f.write_str("missing AutoUpdate= entry in [Container]"),
            Self::IncorrectAutoUpdate { auto_update } => {
                write!(f, "AutoUpdate must remain {REQUIRED_AUTO_UPDATE_POLICY}: {auto_update}")
            }
            Self::MissingApiKeyProvisioningDependency { directive } => {
                DependencyFailure::missing(directive, QDRANT_API_KEY_SERVICE, true).fmt(f)
            }
            Self::IncorrectApiKeyProvisioningDependency { directive, dependency } => {
                DependencyFailure::incorrect(directive, QDRANT_API_KEY_SERVICE, dependency, true)
                    .fmt(f)
            }
            Self::MissingUnitDependency { directive, expected } => {
                DependencyFailure::missing(directive, expected, false).fmt(f)
            }
            Self::IncorrectUnitDependency { directive, expected, actual } => {
                DependencyFailure::incorrect(directive, expected, actual, false).fmt(f)
            }
            Self::MissingApiKeySecret => {
                write!(
                    f,
                    "missing Secret=repovec-qdrant-api-key,type=env,target=QDRANT__SERVICE__API_KEY",
                )
            }
            Self::IncorrectApiKeySecret { secret } => {
                write!(
                    f,
                    "Qdrant API-key secret must be repovec-qdrant-api-key,type=env,target=QDRANT__SERVICE__API_KEY: {secret}",
                )
            }
            Self::InlineApiKeyEnvironmentDisallowed { .. } => f.write_str(
                "Qdrant API keys must use a Podman secret, not inline Environment=: <redacted>",
            ),
        }
    }
}

/// The dependency a `[Unit]` directive failed to declare.
///
/// [`QdrantQuadletError`] carries four variants for these failures because the
/// API-key oneshot's variants are part of the historical public surface and
/// must keep their original messages. Normalising the four into one value keeps
/// the message shapes in one place without collapsing the variants.
struct DependencyFailure<'a> {
    directive: &'a str,
    expected: &'a str,
    /// The wrong unit the directive names, or `None` when it is absent.
    actual: Option<&'a str>,
    is_api_key: bool,
}

impl<'a> DependencyFailure<'a> {
    const fn missing(directive: &'a str, expected: &'a str, is_api_key: bool) -> Self {
        Self { directive, expected, actual: None, is_api_key }
    }

    const fn incorrect(
        directive: &'a str,
        expected: &'a str,
        actual: &'a str,
        is_api_key: bool,
    ) -> Self {
        Self { directive, expected, actual: Some(actual), is_api_key }
    }

    /// Writes the operator-facing message for this failure.
    ///
    /// The API-key wording names the oneshot explicitly and has been stable
    /// since the secret contract shipped, so it is not generalised.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let (directive, expected) = (self.directive, self.expected);
        match (self.is_api_key, self.actual) {
            (true, None) => write!(
                f,
                "missing {directive}={expected} dependency for Qdrant API-key provisioning",
            ),
            (true, Some(actual)) => write!(
                f,
                "{directive} must include {expected} for Qdrant API-key provisioning: {actual}",
            ),
            (false, None) => write!(f, "missing {directive}={expected} dependency"),
            (false, Some(actual)) => write!(f, "{directive} must include {expected}: {actual}"),
        }
    }
}

impl Error for QdrantQuadletError {}
