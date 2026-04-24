//! Semantic validation errors for the Qdrant Quadlet contract.

use std::{error::Error, fmt};

use super::{
    REQUIRED_AUTO_UPDATE_POLICY, REQUIRED_GRPC_PORT, REQUIRED_IMAGE, REQUIRED_REST_PORT,
    REQUIRED_STORAGE_SOURCE, REQUIRED_STORAGE_TARGET,
};

/// Contract failures for the Qdrant Quadlet.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum QdrantQuadletError {
    /// The Quadlet file could not be parsed as a small subset of INI syntax.
    InvalidLine {
        /// The 1-indexed source line number.
        line_number: usize,
        /// The invalid line contents after trimming.
        line: String,
    },
    /// A key-value pair appeared before any section header.
    PropertyBeforeSection {
        /// The 1-indexed source line number.
        line_number: usize,
        /// The misplaced property line after trimming.
        line: String,
    },
    /// No `Image=` entry exists in the `[Container]` section.
    MissingImage,
    /// The image reference is not fully qualified and explicitly versioned.
    ImageNotFullyQualified {
        /// The invalid image reference.
        image: String,
    },
    /// The image reference does not match the pinned project contract.
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
                write!(f, "image reference must be fully qualified and pinned: {image}")
            }
            Self::UnexpectedImage { image } => {
                write!(f, "image reference must remain pinned to {REQUIRED_IMAGE}: {image}")
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
                write!(f, "storage mount must include SELinux relabel :Z: {volume}")
            }
            Self::MissingAutoUpdate => f.write_str("missing AutoUpdate= entry in [Container]"),
            Self::IncorrectAutoUpdate { auto_update } => {
                write!(f, "AutoUpdate must remain {REQUIRED_AUTO_UPDATE_POLICY}: {auto_update}")
            }
        }
    }
}

impl Error for QdrantQuadletError {}
