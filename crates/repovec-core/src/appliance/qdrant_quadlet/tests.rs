//! Unit tests covering the static Qdrant Quadlet contract.

use insta::assert_snapshot;
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
    qdrant_quadlet_contents.replace("docker.io/qdrant/qdrant:v1", "qdrant/qdrant:latest")
}

#[fixture]
fn image_is_duplicated(qdrant_quadlet_contents: String) -> String {
    qdrant_quadlet_contents.replace(
        "Image=docker.io/qdrant/qdrant:v1\n",
        "Image=docker.io/qdrant/qdrant:v1\nImage=docker.io/qdrant/qdrant:v2\n",
    )
}

#[fixture]
fn auto_update_is_duplicated(qdrant_quadlet_contents: String) -> String {
    qdrant_quadlet_contents
        .replace("AutoUpdate=registry\n", "AutoUpdate=registry\nAutoUpdate=local\n")
}

#[fixture]
fn rest_port_has_conflicting_duplicate(qdrant_quadlet_contents: String) -> String {
    qdrant_quadlet_contents.replace(
        "PublishPort=127.0.0.1:6333:6333\n",
        "PublishPort=127.0.0.1:6333:6333\nPublishPort=0.0.0.0:6333:6333\n",
    )
}

#[fixture]
fn invalid_line_in_container_section() -> String {
    // Insert a line with no `=` sign into the [Container] section
    "this-is-not-valid".to_owned()
}

#[fixture]
fn property_before_section() -> String {
    // A key=value line that appears before any section header
    "Image=docker.io/qdrant/qdrant:v1\n[Container]\n".to_owned()
}

#[fixture]
fn image_missing(qdrant_quadlet_contents: String) -> String {
    qdrant_quadlet_contents
        .lines()
        .filter(|l| !l.starts_with("Image="))
        .collect::<Vec<_>>()
        .join("\n")
        + "\n"
}

#[fixture]
fn storage_source_is_wrong(qdrant_quadlet_contents: String) -> String {
    qdrant_quadlet_contents
        .replace("/var/lib/repovec/qdrant-storage", "/var/lib/other/qdrant-storage")
}

#[fixture]
fn selinux_relabel_missing(qdrant_quadlet_contents: String) -> String {
    qdrant_quadlet_contents.replace(":/qdrant/storage:Z", ":/qdrant/storage")
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
fn qdrant_quadlet_rejects_conflicting_rest_port_duplicate(
    rest_port_has_conflicting_duplicate: String,
) {
    let error = validate_qdrant_quadlet(&rest_port_has_conflicting_duplicate)
        .expect_err("conflicting REST publishing should be rejected");

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
fn qdrant_quadlet_rejects_duplicate_auto_update(auto_update_is_duplicated: String) {
    let error = validate_qdrant_quadlet(&auto_update_is_duplicated)
        .expect_err("duplicate auto-update policy should be rejected");

    assert_eq!(
        error,
        QdrantQuadletError::IncorrectAutoUpdate { auto_update: String::from("registry,local") }
    );
}

#[rstest]
fn qdrant_quadlet_rejects_unqualified_images(image_is_unqualified: String) {
    let error = validate_qdrant_quadlet(&image_is_unqualified)
        .expect_err("unqualified images should be rejected");

    assert_eq!(
        error,
        QdrantQuadletError::ImageNotFullyQualified { image: String::from("qdrant/qdrant:latest") }
    );
}

#[rstest]
fn qdrant_quadlet_rejects_duplicate_images(image_is_duplicated: String) {
    let error = validate_qdrant_quadlet(&image_is_duplicated)
        .expect_err("duplicate image values should be rejected");

    assert_eq!(
        error,
        QdrantQuadletError::UnexpectedImage {
            image: String::from("docker.io/qdrant/qdrant:v1,docker.io/qdrant/qdrant:v2"),
        }
    );
}

#[rstest]
fn parser_rejects_invalid_line(invalid_line_in_container_section: String) {
    // Feed a bare [Container] section containing an un-parseable line
    let input = format!("[Container]\n{invalid_line_in_container_section}\n");
    let error = validate_qdrant_quadlet(&input).expect_err("a line without `=` should be rejected");

    assert!(
        matches!(error, QdrantQuadletError::InvalidLine { line_number: 2, .. }),
        "unexpected error: {error:?}",
    );
}

#[rstest]
fn parser_rejects_property_before_section(property_before_section: String) {
    let error = validate_qdrant_quadlet(&property_before_section)
        .expect_err("a key=value before any section header should be rejected");

    assert!(
        matches!(error, QdrantQuadletError::PropertyBeforeSection { line_number: 1, .. }),
        "unexpected error: {error:?}",
    );
}

#[rstest]
fn qdrant_quadlet_requires_image_entry(image_missing: String) {
    let error =
        validate_qdrant_quadlet(&image_missing).expect_err("absent Image= should be rejected");

    assert_eq!(error, QdrantQuadletError::MissingImage);
}

#[rstest]
fn qdrant_quadlet_rejects_wrong_storage_source(storage_source_is_wrong: String) {
    let error = validate_qdrant_quadlet(&storage_source_is_wrong)
        .expect_err("incorrect storage source path should be rejected");

    assert_eq!(
        error,
        QdrantQuadletError::IncorrectStorageSource {
            source: String::from("/var/lib/other/qdrant-storage"),
        }
    );
}

#[rstest]
fn qdrant_quadlet_requires_selinux_relabel(selinux_relabel_missing: String) {
    let error = validate_qdrant_quadlet(&selinux_relabel_missing)
        .expect_err("missing SELinux :Z relabel option should be rejected");

    assert!(
        matches!(error, QdrantQuadletError::MissingSelinuxRelabel { .. }),
        "unexpected error: {error:?}",
    );
}

#[rstest]
#[case::invalid_line(
    QdrantQuadletError::InvalidLine { line_number: 42, line: String::from("bad content") },
    "invalid_line_display"
)]
#[case::property_before_section(
    QdrantQuadletError::PropertyBeforeSection {
        line_number: 1,
        line: String::from("Key=Value"),
    },
    "property_before_section_display"
)]
#[case::missing_image(QdrantQuadletError::MissingImage, "missing_image_display")]
#[case::image_not_fully_qualified(
    QdrantQuadletError::ImageNotFullyQualified { image: String::from("qdrant/qdrant:latest") },
    "image_not_fully_qualified_display"
)]
#[case::unexpected_image(
    QdrantQuadletError::UnexpectedImage { image: String::from("docker.io/other/image:v2") },
    "unexpected_image_display"
)]
#[case::missing_rest_port(QdrantQuadletError::MissingRestPort, "missing_rest_port_display")]
#[case::missing_grpc_port(QdrantQuadletError::MissingGrpcPort, "missing_grpc_port_display")]
#[case::port_not_bound_to_loopback(
    QdrantQuadletError::PortNotBoundToLoopback {
        port: 6333,
        publish_port: String::from("0.0.0.0:6333:6333"),
    },
    "port_not_bound_to_loopback_display"
)]
#[case::missing_storage_mount(
    QdrantQuadletError::MissingStorageMount,
    "missing_storage_mount_display"
)]
#[case::incorrect_storage_source(
    QdrantQuadletError::IncorrectStorageSource { source: String::from("/wrong/path") },
    "incorrect_storage_source_display"
)]
#[case::incorrect_storage_target(
    QdrantQuadletError::IncorrectStorageTarget { target: String::from("/srv/qdrant") },
    "incorrect_storage_target_display"
)]
#[case::missing_selinux_relabel(
    QdrantQuadletError::MissingSelinuxRelabel { volume: String::from("/data:/qdrant/storage") },
    "missing_selinux_relabel_display"
)]
#[case::missing_auto_update(QdrantQuadletError::MissingAutoUpdate, "missing_auto_update_display")]
#[case::incorrect_auto_update(
    QdrantQuadletError::IncorrectAutoUpdate { auto_update: String::from("local") },
    "incorrect_auto_update_display"
)]
fn qdrant_quadlet_error_display_matches_snapshot(
    #[case] error: QdrantQuadletError,
    #[case] snapshot_label: &'static str,
) {
    assert_snapshot!(snapshot_label, error.to_string());
}
