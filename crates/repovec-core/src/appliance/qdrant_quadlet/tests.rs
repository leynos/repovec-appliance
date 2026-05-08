//! Contract tests for `validate_qdrant_quadlet`: deterministic mutations of the
//! shipped quadlet (`checked_in_qdrant_quadlet`) plus committed `insta`
//! diagnostics colocated under `snapshots/`. Behavioural scenarios also live
//! in `crates/repovec-core/tests/qdrant_quadlet_bdd.rs`.

use insta::assert_snapshot;
use rstest::{fixture, rstest};

use super::{
    QdrantQuadletError, checked_in_qdrant_quadlet, validate_checked_in_qdrant_quadlet,
    validate_qdrant_quadlet,
};

/// Mutations of the checked-in Quadlet used to reach distinct validation errors.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ValidationScenario {
    InvalidLineInContainer,
    PropertyBeforeFirstSection,
    MissingImage,
    ImageUnqualified,
    WrongFullyQualifiedImage,
    DuplicateImageValues,
    MissingRestPublish,
    MissingGrpcPublish,
    RestPublishNotLoopback,
    ConflictingRestPublishDuplicate,
    GrpcPublishNotLoopback,
    StorageVolumeMissing,
    StorageSourceWrong,
    StorageTargetWrong,
    SelinuxRelabelMissing,
    AutoUpdateMissing,
    AutoUpdateWrongValue,
    DuplicateAutoUpdateValues,
    RestPublishMalformed,
    GrpcPublishMalformed,
    VolumeLineWithoutMountContract,
}

impl ValidationScenario {
    fn mutated_contents(self, canonical: &str) -> String {
        match self {
            Self::InvalidLineInContainer => String::from("[Container]\nthis-is-not-valid\n"),
            Self::PropertyBeforeFirstSection => {
                String::from("Image=docker.io/qdrant/qdrant:v1\n[Container]\n")
            }
            Self::MissingImage => {
                canonical
                    .lines()
                    .filter(|line| !line.starts_with("Image="))
                    .collect::<Vec<_>>()
                    .join("\n")
                    + "\n"
            }
            Self::ImageUnqualified => {
                canonical.replace("docker.io/qdrant/qdrant:v1", "qdrant/qdrant:latest")
            }
            Self::WrongFullyQualifiedImage => {
                canonical.replace("docker.io/qdrant/qdrant:v1", "docker.io/other/image:v2")
            }
            Self::DuplicateImageValues => {
                canonical.replace(
                    "Image=docker.io/qdrant/qdrant:v1\n",
                    concat!(
                        "Image=docker.io/qdrant/qdrant:v1\n",
                        "Image=docker.io/qdrant/qdrant:v2\n",
                    ),
                )
            }
            Self::MissingRestPublish => canonical.replace("PublishPort=127.0.0.1:6333:6333\n", ""),
            Self::MissingGrpcPublish => canonical.replace("PublishPort=127.0.0.1:6334:6334\n", ""),
            Self::RestPublishNotLoopback => {
                canonical.replace("127.0.0.1:6333:6333", "0.0.0.0:6333:6333")
            }
            Self::ConflictingRestPublishDuplicate => canonical.replace(
                "PublishPort=127.0.0.1:6333:6333\n",
                concat!("PublishPort=127.0.0.1:6333:6333\n", "PublishPort=0.0.0.0:6333:6333\n",),
            ),
            Self::GrpcPublishNotLoopback => {
                canonical.replace("127.0.0.1:6334:6334", "0.0.0.0:6334:6334")
            }
            Self::StorageVolumeMissing => {
                canonical.replace("Volume=/var/lib/repovec/qdrant-storage:/qdrant/storage:Z\n", "")
            }
            Self::StorageSourceWrong => canonical
                .replace("/var/lib/repovec/qdrant-storage", "/var/lib/other/qdrant-storage"),
            Self::StorageTargetWrong => canonical.replace("/qdrant/storage:Z", "/srv/qdrant:Z"),
            Self::SelinuxRelabelMissing => {
                canonical.replace(":/qdrant/storage:Z", ":/qdrant/storage")
            }
            Self::AutoUpdateMissing => canonical.replace("AutoUpdate=registry\n", ""),
            Self::AutoUpdateWrongValue => {
                canonical.replace("AutoUpdate=registry\n", "AutoUpdate=local\n")
            }
            Self::DuplicateAutoUpdateValues => canonical.replace(
                "AutoUpdate=registry\n",
                concat!("AutoUpdate=registry\n", "AutoUpdate=local\n"),
            ),
            Self::RestPublishMalformed => canonical
                .replace("PublishPort=127.0.0.1:6333:6333\n", "PublishPort=not-a-mapping\n"),
            Self::GrpcPublishMalformed => canonical.replace(
                "PublishPort=127.0.0.1:6334:6334\n",
                "PublishPort=still-not-three-fields\n",
            ),
            Self::VolumeLineWithoutMountContract => canonical.replace(
                "Volume=/var/lib/repovec/qdrant-storage:/qdrant/storage:Z\n",
                "Volume=/tmp/nothing-to-do-with-qdrant:/tmp/other:Z\n",
            ),
        }
    }

    fn snapshot_label(self) -> &'static str {
        match self {
            Self::InvalidLineInContainer => "invalid_line_display",
            Self::PropertyBeforeFirstSection => "property_before_section_display",
            Self::MissingImage => "missing_image_display",
            Self::ImageUnqualified => "image_not_fully_qualified_display",
            Self::WrongFullyQualifiedImage => "unexpected_image_display",
            Self::DuplicateImageValues => "duplicate_image_values_display",
            Self::MissingRestPublish | Self::RestPublishMalformed => "missing_rest_port_display",
            Self::MissingGrpcPublish | Self::GrpcPublishMalformed => "missing_grpc_port_display",
            Self::RestPublishNotLoopback | Self::ConflictingRestPublishDuplicate => {
                "port_not_bound_to_loopback_display"
            }
            Self::GrpcPublishNotLoopback => "grpc_port_not_loopback_display",
            Self::StorageVolumeMissing | Self::VolumeLineWithoutMountContract => {
                "missing_storage_mount_display"
            }
            Self::StorageSourceWrong => "incorrect_storage_source_display",
            Self::StorageTargetWrong => "incorrect_storage_target_display",
            Self::SelinuxRelabelMissing => "missing_selinux_relabel_display",
            Self::AutoUpdateMissing => "missing_auto_update_display",
            Self::AutoUpdateWrongValue => "incorrect_auto_update_display",
            Self::DuplicateAutoUpdateValues => "duplicate_auto_update_policies_display",
        }
    }

    fn expected_error(self) -> QdrantQuadletError {
        match self {
            Self::InvalidLineInContainer => QdrantQuadletError::InvalidLine {
                line_number: 2,
                line: String::from("this-is-not-valid"),
            },
            Self::PropertyBeforeFirstSection => QdrantQuadletError::PropertyBeforeSection {
                line_number: 1,
                line: String::from("Image=docker.io/qdrant/qdrant:v1"),
            },
            Self::MissingImage => QdrantQuadletError::MissingImage,
            Self::ImageUnqualified => QdrantQuadletError::ImageNotFullyQualified {
                image: String::from("qdrant/qdrant:latest"),
            },
            Self::WrongFullyQualifiedImage => QdrantQuadletError::UnexpectedImage {
                image: String::from("docker.io/other/image:v2"),
            },
            Self::DuplicateImageValues => QdrantQuadletError::UnexpectedImage {
                image: String::from("docker.io/qdrant/qdrant:v1,docker.io/qdrant/qdrant:v2"),
            },
            Self::MissingRestPublish | Self::RestPublishMalformed => {
                QdrantQuadletError::MissingRestPort
            }
            Self::MissingGrpcPublish | Self::GrpcPublishMalformed => {
                QdrantQuadletError::MissingGrpcPort
            }
            Self::RestPublishNotLoopback | Self::ConflictingRestPublishDuplicate => {
                QdrantQuadletError::PortNotBoundToLoopback {
                    port: 6333,
                    publish_port: String::from("0.0.0.0:6333:6333"),
                }
            }
            Self::GrpcPublishNotLoopback => QdrantQuadletError::PortNotBoundToLoopback {
                port: 6334,
                publish_port: String::from("0.0.0.0:6334:6334"),
            },
            Self::StorageVolumeMissing | Self::VolumeLineWithoutMountContract => {
                QdrantQuadletError::MissingStorageMount
            }
            Self::StorageSourceWrong => QdrantQuadletError::IncorrectStorageSource {
                source: String::from("/var/lib/other/qdrant-storage"),
            },
            Self::StorageTargetWrong => {
                QdrantQuadletError::IncorrectStorageTarget { target: String::from("/srv/qdrant") }
            }
            Self::SelinuxRelabelMissing => QdrantQuadletError::MissingSelinuxRelabel {
                volume: String::from("/var/lib/repovec/qdrant-storage:/qdrant/storage"),
            },
            Self::AutoUpdateMissing => QdrantQuadletError::MissingAutoUpdate,
            Self::AutoUpdateWrongValue => {
                QdrantQuadletError::IncorrectAutoUpdate { auto_update: String::from("local") }
            }
            Self::DuplicateAutoUpdateValues => QdrantQuadletError::IncorrectAutoUpdate {
                auto_update: String::from("registry,local"),
            },
        }
    }
}

#[fixture]
#[rustfmt::skip]
fn qdrant_quadlet_contents() -> String {
    // Rustfmt collapsing this body to one line triggers `unused-braces`; keep an
    // explicit block despite the terse body.
    checked_in_qdrant_quadlet().to_owned()
}

#[test]
fn checked_in_qdrant_quadlet_remains_valid() {
    validate_checked_in_qdrant_quadlet()
        .expect("the checked-in Qdrant Quadlet should remain valid");
}

#[rstest]
#[case::invalid_line(ValidationScenario::InvalidLineInContainer)]
#[case::property_before_section(ValidationScenario::PropertyBeforeFirstSection)]
#[case::missing_image(ValidationScenario::MissingImage)]
#[case::image_unqualified(ValidationScenario::ImageUnqualified)]
#[case::wrong_fully_qualified_image(ValidationScenario::WrongFullyQualifiedImage)]
#[case::duplicate_image_values(ValidationScenario::DuplicateImageValues)]
#[case::missing_rest_publish(ValidationScenario::MissingRestPublish)]
#[case::malformed_rest_publish(ValidationScenario::RestPublishMalformed)]
#[case::missing_grpc_publish(ValidationScenario::MissingGrpcPublish)]
#[case::malformed_grpc_publish(ValidationScenario::GrpcPublishMalformed)]
#[case::rest_port_not_loopback(ValidationScenario::RestPublishNotLoopback)]
#[case::conflicting_rest_publish(ValidationScenario::ConflictingRestPublishDuplicate)]
#[case::grpc_port_not_loopback(ValidationScenario::GrpcPublishNotLoopback)]
#[case::storage_mount_missing(ValidationScenario::StorageVolumeMissing)]
#[case::volume_unrelated_to_contract(ValidationScenario::VolumeLineWithoutMountContract)]
#[case::storage_source_wrong(ValidationScenario::StorageSourceWrong)]
#[case::storage_target_wrong(ValidationScenario::StorageTargetWrong)]
#[case::selinux_relabel_missing(ValidationScenario::SelinuxRelabelMissing)]
#[case::auto_update_missing(ValidationScenario::AutoUpdateMissing)]
#[case::auto_update_wrong_value(ValidationScenario::AutoUpdateWrongValue)]
#[case::duplicate_auto_update(ValidationScenario::DuplicateAutoUpdateValues)]
fn validated_qdrant_quadlet_violations_match_expected_variant_and_diagnostic_snapshots(
    qdrant_quadlet_contents: String,
    #[case] scenario: ValidationScenario,
) {
    let input = scenario.mutated_contents(&qdrant_quadlet_contents);
    let Err(err) = validate_qdrant_quadlet(&input) else {
        panic!(
            "expected {scenario:?} validation to fail — input parsed as valid Quadlet:\n---\n{input}\n---",
        );
    };

    assert_eq!(err, scenario.expected_error());
    assert_snapshot!(scenario.snapshot_label(), err.to_string());
}
