//! Validation helpers for the checked-in Qdrant Podman Quadlet asset.

use std::{collections::BTreeMap, error::Error, fmt};

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

#[derive(Debug)]
struct ParsedQuadlet {
    sections: BTreeMap<String, BTreeMap<String, Vec<String>>>,
}

impl ParsedQuadlet {
    fn parse(contents: &str) -> Result<Self, QdrantQuadletError> {
        let mut sections = BTreeMap::<String, BTreeMap<String, Vec<String>>>::new();
        let mut current_section: Option<String> = None;

        for (line_index, raw_line) in contents.lines().enumerate() {
            let line_number = line_index + 1;
            let line = raw_line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }

            if let Some(section) = parse_section_header(line) {
                current_section = Some(section.to_owned());
                sections.entry(section.to_owned()).or_default();
                continue;
            }

            let Some((key, value)) = line.split_once('=') else {
                return Err(QdrantQuadletError::InvalidLine { line_number, line: line.to_owned() });
            };

            let Some(section) = &current_section else {
                return Err(QdrantQuadletError::PropertyBeforeSection {
                    line_number,
                    line: line.to_owned(),
                });
            };

            sections
                .entry(section.clone())
                .or_default()
                .entry(key.trim().to_owned())
                .or_default()
                .push(value.trim().to_owned());
        }

        Ok(Self { sections })
    }

    fn values(&self, section: &str, key: &str) -> &[String] {
        self.sections.get(section).and_then(|entries| entries.get(key)).map_or(&[], Vec::as_slice)
    }
}

fn parse_section_header(line: &str) -> Option<&str> { line.strip_prefix('[')?.strip_suffix(']') }

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

#[cfg(test)]
mod tests {
    //! Unit tests covering the static Qdrant Quadlet contract.

    use rstest::{fixture, rstest};

    use super::{
        QdrantQuadletError, checked_in_qdrant_quadlet, validate_checked_in_qdrant_quadlet,
        validate_qdrant_quadlet,
    };

    #[fixture]
    fn qdrant_quadlet_contents() -> String {
        let mut contents = String::new();
        contents.push_str(checked_in_qdrant_quadlet());
        contents
    }

    #[fixture]
    fn rest_port_bound_wildcard(qdrant_quadlet_contents: String) -> String {
        qdrant_quadlet_contents.replace("127.0.0.1:6333:6333", "0.0.0.0:6333:6333")
    }

    #[fixture]
    fn grpc_port_missing(qdrant_quadlet_contents: String) -> String {
        qdrant_quadlet_contents.replace("PublishPort=127.0.0.1:6334:6334\n", "")
    }

    #[fixture]
    fn storage_mount_missing(qdrant_quadlet_contents: String) -> String {
        qdrant_quadlet_contents
            .replace("Volume=/var/lib/repovec/qdrant-storage:/qdrant/storage:Z\n", "")
    }

    #[fixture]
    fn storage_target_is_wrong(qdrant_quadlet_contents: String) -> String {
        qdrant_quadlet_contents.replace("/qdrant/storage:Z", "/srv/qdrant:Z")
    }

    #[fixture]
    fn auto_update_missing(qdrant_quadlet_contents: String) -> String {
        qdrant_quadlet_contents.replace("AutoUpdate=registry\n", "")
    }

    #[fixture]
    fn image_is_unqualified(qdrant_quadlet_contents: String) -> String {
        qdrant_quadlet_contents.replace("docker.io/qdrant/qdrant:v1.17.1", "qdrant/qdrant:latest")
    }

    #[test]
    fn checked_in_qdrant_quadlet_remains_valid() {
        validate_checked_in_qdrant_quadlet()
            .expect("the checked-in Qdrant Quadlet should remain valid");
    }

    #[rstest]
    fn qdrant_quadlet_rejects_rest_port_without_loopback(rest_port_bound_wildcard: String) {
        let error = validate_qdrant_quadlet(&rest_port_bound_wildcard)
            .expect_err("wildcard REST publishing should be rejected");

        assert_eq!(
            error,
            QdrantQuadletError::PortNotBoundToLoopback {
                port: 6333,
                publish_port: String::from("0.0.0.0:6333:6333"),
            }
        );
    }

    #[rstest]
    fn qdrant_quadlet_requires_grpc_port(grpc_port_missing: String) {
        let error = validate_qdrant_quadlet(&grpc_port_missing)
            .expect_err("missing gRPC publishing should be rejected");

        assert_eq!(error, QdrantQuadletError::MissingGrpcPort);
    }

    #[rstest]
    fn qdrant_quadlet_requires_storage_mount(storage_mount_missing: String) {
        let error = validate_qdrant_quadlet(&storage_mount_missing)
            .expect_err("missing storage mount should be rejected");

        assert_eq!(error, QdrantQuadletError::MissingStorageMount);
    }

    #[rstest]
    fn qdrant_quadlet_requires_expected_storage_target(storage_target_is_wrong: String) {
        let error = validate_qdrant_quadlet(&storage_target_is_wrong)
            .expect_err("wrong storage target should be rejected");

        assert_eq!(
            error,
            QdrantQuadletError::IncorrectStorageTarget { target: String::from("/srv/qdrant") }
        );
    }

    #[rstest]
    fn qdrant_quadlet_requires_auto_update(auto_update_missing: String) {
        let error = validate_qdrant_quadlet(&auto_update_missing)
            .expect_err("missing auto-update should be rejected");

        assert_eq!(error, QdrantQuadletError::MissingAutoUpdate);
    }

    #[rstest]
    fn qdrant_quadlet_rejects_unqualified_images(image_is_unqualified: String) {
        let error = validate_qdrant_quadlet(&image_is_unqualified)
            .expect_err("unqualified images should be rejected");

        assert_eq!(
            error,
            QdrantQuadletError::ImageNotFullyQualified {
                image: String::from("qdrant/qdrant:latest"),
            }
        );
    }
}
