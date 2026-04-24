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
        QdrantQuadletError::ImageNotFullyQualified { image: String::from("qdrant/qdrant:latest") }
    );
}
