//! Unit tests covering Qdrant API-key-specific Quadlet validation.

use rstest::rstest;

use super::{QdrantQuadletError, checked_in_qdrant_quadlet, validate_qdrant_quadlet};

fn qdrant_quadlet_contents() -> String { checked_in_qdrant_quadlet().to_owned() }

fn quoted_inline_api_key_environment() -> String {
    qdrant_quadlet_contents().replace(
        "Secret=repovec-qdrant-api-key,type=env,target=QDRANT__SERVICE__API_KEY\n",
        concat!(
            "Secret=repovec-qdrant-api-key,type=env,target=QDRANT__SERVICE__API_KEY\n",
            "Environment=\"QDRANT__SERVICE__API_KEY=secret\"\n",
        ),
    )
}

#[test]
fn api_key_error_display_messages_remain_stable() {
    insta::assert_snapshot!(
        QdrantQuadletError::MissingApiKeyProvisioningDependency { directive: "Requires" }
            .to_string(),
        @"missing Requires=repovec-qdrant-api-key.service dependency for Qdrant API-key provisioning"
    );
    insta::assert_snapshot!(
        QdrantQuadletError::IncorrectApiKeyProvisioningDependency {
            directive: "After",
            dependency: String::from("network-online.target"),
        }
        .to_string(),
        @"After must include repovec-qdrant-api-key.service for Qdrant API-key provisioning: network-online.target"
    );
    insta::assert_snapshot!(
        QdrantQuadletError::MissingApiKeySecret.to_string(),
        @"missing Secret=repovec-qdrant-api-key,type=env,target=QDRANT__SERVICE__API_KEY"
    );
    insta::assert_snapshot!(
        QdrantQuadletError::IncorrectApiKeySecret {
            secret: String::from("repovec-qdrant-api-key,type=mount,target=QDRANT__SERVICE__API_KEY"),
        }
        .to_string(),
        @"Qdrant API-key secret must be repovec-qdrant-api-key,type=env,target=QDRANT__SERVICE__API_KEY: repovec-qdrant-api-key,type=mount,target=QDRANT__SERVICE__API_KEY"
    );
    insta::assert_snapshot!(
        QdrantQuadletError::InlineApiKeyEnvironmentDisallowed {
            environment: String::from("QDRANT__SERVICE__API_KEY=plaintext"),
        }
        .to_string(),
        @"Qdrant API keys must use a Podman secret, not inline Environment=: <redacted>"
    );
}

fn multi_assignment_inline_api_key_environment() -> String {
    qdrant_quadlet_contents().replace(
        "Secret=repovec-qdrant-api-key,type=env,target=QDRANT__SERVICE__API_KEY\n",
        concat!(
            "Secret=repovec-qdrant-api-key,type=env,target=QDRANT__SERVICE__API_KEY\n",
            "Environment=FOO=bar QDRANT__SERVICE__API_KEY=secret\n",
        ),
    )
}

fn api_key_secret_missing() -> String {
    qdrant_quadlet_contents()
        .replace("Secret=repovec-qdrant-api-key,type=env,target=QDRANT__SERVICE__API_KEY\n", "")
}

fn api_key_secret_name_is_wrong() -> String {
    qdrant_quadlet_contents().replace("Secret=repovec-qdrant-api-key,", "Secret=qdrant-key,")
}

fn api_key_secret_target_is_wrong() -> String {
    qdrant_quadlet_contents()
        .replace("target=QDRANT__SERVICE__API_KEY", "target=QDRANT__SERVICE__READ_ONLY_API_KEY")
}

fn api_key_secret_type_is_wrong() -> String {
    qdrant_quadlet_contents().replace(
        "Secret=repovec-qdrant-api-key,type=env,target=QDRANT__SERVICE__API_KEY\n",
        "Secret=repovec-qdrant-api-key,type=mount,target=QDRANT__SERVICE__API_KEY\n",
    )
}

fn api_key_requires_dependency_missing() -> String {
    qdrant_quadlet_contents().replace("Requires=repovec-qdrant-api-key.service\n", "")
}

fn api_key_after_dependency_missing() -> String {
    qdrant_quadlet_contents().replace("After=repovec-qdrant-api-key.service\n", "")
}

fn api_key_requires_dependency_wrong() -> String {
    qdrant_quadlet_contents()
        .replace("Requires=repovec-qdrant-api-key.service", "Requires=network-online.target")
}

fn api_key_after_dependency_wrong() -> String {
    qdrant_quadlet_contents()
        .replace("After=repovec-qdrant-api-key.service", "After=network-online.target")
}

fn inline_api_key_environment() -> String {
    qdrant_quadlet_contents().replace(
        "Secret=repovec-qdrant-api-key,type=env,target=QDRANT__SERVICE__API_KEY\n",
        concat!(
            "Secret=repovec-qdrant-api-key,type=env,target=QDRANT__SERVICE__API_KEY\n",
            "Environment=QDRANT__SERVICE__API_KEY=not-secret\n",
        ),
    )
}

#[test]
fn qdrant_quadlet_requires_api_key_secret() {
    let contents = api_key_secret_missing();
    let error =
        validate_qdrant_quadlet(&contents).expect_err("missing API-key secret should be rejected");

    assert_eq!(error, QdrantQuadletError::MissingApiKeySecret);
}

#[test]
fn qdrant_quadlet_requires_api_key_secret_name() {
    let contents = api_key_secret_name_is_wrong();
    let error = validate_qdrant_quadlet(&contents)
        .expect_err("wrong API-key secret name should be rejected");

    assert_eq!(
        error,
        QdrantQuadletError::IncorrectApiKeySecret {
            secret: String::from("qdrant-key,type=env,target=QDRANT__SERVICE__API_KEY"),
        }
    );
}

#[test]
fn qdrant_quadlet_requires_api_key_secret_target() {
    let contents = api_key_secret_target_is_wrong();
    let error = validate_qdrant_quadlet(&contents)
        .expect_err("wrong API-key secret target should be rejected");

    assert_eq!(
        error,
        QdrantQuadletError::IncorrectApiKeySecret {
            secret: String::from(
                "repovec-qdrant-api-key,type=env,target=QDRANT__SERVICE__READ_ONLY_API_KEY",
            ),
        }
    );
}

#[test]
fn qdrant_quadlet_requires_api_key_secret_env_type() {
    let contents = api_key_secret_type_is_wrong();
    let error = validate_qdrant_quadlet(&contents)
        .expect_err("wrong API-key secret type should be rejected");

    assert_eq!(
        error,
        QdrantQuadletError::IncorrectApiKeySecret {
            secret: String::from(
                "repovec-qdrant-api-key,type=mount,target=QDRANT__SERVICE__API_KEY",
            ),
        }
    );
}

#[test]
fn qdrant_quadlet_requires_api_key_requires_dependency() {
    let contents = api_key_requires_dependency_missing();
    let error = validate_qdrant_quadlet(&contents)
        .expect_err("missing API-key Requires= dependency should be rejected");

    assert_eq!(
        error,
        QdrantQuadletError::MissingApiKeyProvisioningDependency { directive: "Requires" }
    );
}

#[test]
fn qdrant_quadlet_requires_api_key_after_dependency() {
    let contents = api_key_after_dependency_missing();
    let error = validate_qdrant_quadlet(&contents)
        .expect_err("missing API-key After= dependency should be rejected");

    assert_eq!(
        error,
        QdrantQuadletError::MissingApiKeyProvisioningDependency { directive: "After" }
    );
}

#[test]
fn qdrant_quadlet_rejects_wrong_api_key_dependency() {
    let contents = api_key_requires_dependency_wrong();
    let error = validate_qdrant_quadlet(&contents)
        .expect_err("wrong API-key dependency should be rejected");

    assert_eq!(
        error,
        QdrantQuadletError::IncorrectApiKeyProvisioningDependency {
            directive: "Requires",
            dependency: String::from("network-online.target"),
        }
    );
}

#[test]
fn qdrant_quadlet_rejects_wrong_api_key_after_dependency() {
    let contents = api_key_after_dependency_wrong();
    let error = validate_qdrant_quadlet(&contents)
        .expect_err("wrong API-key After= dependency should be rejected");

    assert_eq!(
        error,
        QdrantQuadletError::IncorrectApiKeyProvisioningDependency {
            directive: "After",
            dependency: String::from("network-online.target"),
        }
    );
}

#[test]
fn qdrant_quadlet_rejects_inline_api_key_environment() {
    let contents = inline_api_key_environment();
    let error = validate_qdrant_quadlet(&contents).expect_err("inline API keys should be rejected");

    assert_eq!(
        error,
        QdrantQuadletError::InlineApiKeyEnvironmentDisallowed {
            environment: String::from("QDRANT__SERVICE__API_KEY=not-secret"),
        }
    );
}

#[rstest]
#[case(quoted_inline_api_key_environment(), "QDRANT__SERVICE__API_KEY=secret")]
#[case(multi_assignment_inline_api_key_environment(), "QDRANT__SERVICE__API_KEY=secret")]
fn qdrant_quadlet_rejects_shell_tokenized_inline_api_key_environment(
    #[case] contents: String,
    #[case] expected_assignment: &str,
) {
    let error = validate_qdrant_quadlet(&contents)
        .expect_err("inline API keys should be rejected after tokenization");

    assert_eq!(
        error,
        QdrantQuadletError::InlineApiKeyEnvironmentDisallowed {
            environment: expected_assignment.to_owned(),
        }
    );
}
