//! Behavioural tests for the checked-in Qdrant Quadlet contract.

use repovec_core::appliance::qdrant_quadlet::{
    QdrantQuadletError, checked_in_qdrant_quadlet, validate_qdrant_quadlet,
};
use rstest::fixture;
use rstest_bdd_macros::{given, scenario, then, when};

#[derive(Default)]
struct QuadletWorld {
    contents: String,
    validation_result: Option<Result<(), QdrantQuadletError>>,
}

#[fixture]
fn quadlet_world() -> QuadletWorld {
    let validation_result = None;
    QuadletWorld { contents: String::new(), validation_result }
}

#[given("the checked-in Qdrant Quadlet")]
fn the_checked_in_qdrant_quadlet(quadlet_world: &mut QuadletWorld) {
    checked_in_qdrant_quadlet().clone_into(&mut quadlet_world.contents);
}

#[given("the REST port is published on 0.0.0.0")]
fn the_rest_port_is_published_on_wildcard(quadlet_world: &mut QuadletWorld) {
    quadlet_world.contents =
        quadlet_world.contents.replace("127.0.0.1:6333:6333", "0.0.0.0:6333:6333");
}

#[given("the gRPC port mapping is removed")]
fn the_grpc_port_mapping_is_removed(quadlet_world: &mut QuadletWorld) {
    quadlet_world.contents =
        quadlet_world.contents.replace("PublishPort=127.0.0.1:6334:6334\n", "");
}

#[given("the persistent storage mount is removed")]
fn the_persistent_storage_mount_is_removed(quadlet_world: &mut QuadletWorld) {
    quadlet_world.contents = quadlet_world
        .contents
        .replace("Volume=/var/lib/repovec/qdrant-storage:/qdrant/storage:Z\n", "");
}

#[given("Podman auto-update is removed")]
fn podman_auto_update_is_removed(quadlet_world: &mut QuadletWorld) {
    quadlet_world.contents = quadlet_world.contents.replace("AutoUpdate=registry\n", "");
}

#[given("the API key secret is removed")]
fn the_api_key_secret_is_removed(quadlet_world: &mut QuadletWorld) {
    quadlet_world.contents = quadlet_world
        .contents
        .replace("Secret=repovec-qdrant-api-key,type=env,target=QDRANT__SERVICE__API_KEY\n", "");
}

#[given("the API key is inlined as an environment variable")]
fn the_api_key_is_inlined_as_an_environment_variable(quadlet_world: &mut QuadletWorld) {
    quadlet_world.contents = quadlet_world.contents.replace(
        "Secret=repovec-qdrant-api-key,type=env,target=QDRANT__SERVICE__API_KEY\n",
        concat!(
            "Secret=repovec-qdrant-api-key,type=env,target=QDRANT__SERVICE__API_KEY\n",
            "Environment=QDRANT__SERVICE__API_KEY=not-secret\n",
        ),
    );
}

#[when("the Quadlet is validated")]
fn the_quadlet_is_validated(quadlet_world: &mut QuadletWorld) {
    quadlet_world.validation_result = Some(validate_qdrant_quadlet(&quadlet_world.contents));
}

#[then("the Quadlet is accepted")]
fn the_quadlet_is_accepted(quadlet_world: &QuadletWorld) {
    let Some(validation_result) = quadlet_world.validation_result.as_ref() else {
        panic!("the validation step should have run");
    };

    assert!(validation_result.is_ok());
}

#[then("the validation fails with a loopback error for port 6333")]
fn the_validation_fails_with_a_loopback_error_for_port_6333(quadlet_world: &QuadletWorld) {
    let Some(validation_result) = quadlet_world.validation_result.as_ref() else {
        panic!("the validation step should have run");
    };

    assert_eq!(
        validation_result,
        &Err(QdrantQuadletError::PortNotBoundToLoopback {
            port: 6333,
            publish_port: String::from("0.0.0.0:6333:6333"),
        })
    );
}

#[then("the validation fails because the gRPC port is missing")]
fn the_validation_fails_because_the_grpc_port_is_missing(quadlet_world: &QuadletWorld) {
    let Some(validation_result) = quadlet_world.validation_result.as_ref() else {
        panic!("the validation step should have run");
    };

    assert_eq!(validation_result, &Err(QdrantQuadletError::MissingGrpcPort));
}

#[then("the validation fails because the storage mount is missing")]
fn the_validation_fails_because_the_storage_mount_is_missing(quadlet_world: &QuadletWorld) {
    let Some(validation_result) = quadlet_world.validation_result.as_ref() else {
        panic!("the validation step should have run");
    };

    assert_eq!(validation_result, &Err(QdrantQuadletError::MissingStorageMount));
}

#[then("the validation fails because auto-update is missing")]
fn the_validation_fails_because_auto_update_is_missing(quadlet_world: &QuadletWorld) {
    let Some(validation_result) = quadlet_world.validation_result.as_ref() else {
        panic!("the validation step should have run");
    };

    assert_eq!(validation_result, &Err(QdrantQuadletError::MissingAutoUpdate));
}

#[then("the validation fails because the API key secret is missing")]
fn the_validation_fails_because_the_api_key_secret_is_missing(quadlet_world: &QuadletWorld) {
    let Some(validation_result) = quadlet_world.validation_result.as_ref() else {
        panic!("the validation step should have run");
    };

    assert_eq!(validation_result, &Err(QdrantQuadletError::MissingApiKeySecret));
}

#[then("the validation fails because inline API keys are not allowed")]
fn the_validation_fails_because_inline_api_keys_are_not_allowed(quadlet_world: &QuadletWorld) {
    let Some(validation_result) = quadlet_world.validation_result.as_ref() else {
        panic!("the validation step should have run");
    };

    assert_eq!(
        validation_result,
        &Err(QdrantQuadletError::InlineApiKeyEnvironmentDisallowed {
            environment: String::from("QDRANT__SERVICE__API_KEY=not-secret"),
        })
    );
}

#[scenario(
    path = "tests/features/qdrant_quadlet.feature",
    name = "The checked-in Quadlet satisfies the appliance contract"
)]
fn checked_in_quadlet_satisfies_the_appliance_contract(quadlet_world: QuadletWorld) {
    let _ = quadlet_world;
}

#[scenario(
    path = "tests/features/qdrant_quadlet.feature",
    name = "The REST port remains loopback-only"
)]
fn rest_port_remains_loopback_only(quadlet_world: QuadletWorld) { let _ = quadlet_world; }

#[scenario(path = "tests/features/qdrant_quadlet.feature", name = "The gRPC port must be present")]
fn grpc_port_must_be_present(quadlet_world: QuadletWorld) { let _ = quadlet_world; }

#[scenario(
    path = "tests/features/qdrant_quadlet.feature",
    name = "Persistent storage remains mounted"
)]
fn persistent_storage_remains_mounted(quadlet_world: QuadletWorld) { let _ = quadlet_world; }

#[scenario(
    path = "tests/features/qdrant_quadlet.feature",
    name = "Podman auto-update remains enabled"
)]
fn podman_auto_update_remains_enabled(quadlet_world: QuadletWorld) { let _ = quadlet_world; }

#[scenario(
    path = "tests/features/qdrant_quadlet.feature",
    name = "The Qdrant API key secret must be present"
)]
fn qdrant_api_key_secret_must_be_present(quadlet_world: QuadletWorld) { let _ = quadlet_world; }

#[scenario(
    path = "tests/features/qdrant_quadlet.feature",
    name = "Inline Qdrant API keys are rejected"
)]
fn inline_qdrant_api_keys_are_rejected(quadlet_world: QuadletWorld) { let _ = quadlet_world; }
