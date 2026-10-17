//! Unit tests covering Qdrant API-key-specific Quadlet validation.
//!
//! These tests exercise API-key-specific validation paths of
//! `validate_qdrant_quadlet`, which is defined in `mod.rs`. Display-stability
//! snapshots here complement the broader Quadlet contract snapshots in the
//! sibling `tests.rs` module.

use insta::assert_snapshot;
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

fn api_key_secret_type_is_contradictory() -> String {
    qdrant_quadlet_contents().replace(
        "Secret=repovec-qdrant-api-key,type=env,target=QDRANT__SERVICE__API_KEY\n",
        "Secret=repovec-qdrant-api-key,type=env,type=mount,target=QDRANT__SERVICE__API_KEY\n",
    )
}

fn api_key_requires_dependency_missing() -> String {
    qdrant_quadlet_contents()
        .replace("Requires=repovec-qdrant-api-key.service repovec-provision.service\n", "")
}

fn api_key_after_dependency_missing() -> String {
    qdrant_quadlet_contents().replace("After=repovec-qdrant-api-key.service\n", "")
}

fn api_key_requires_dependency_wrong() -> String {
    qdrant_quadlet_contents().replace(
        "Requires=repovec-qdrant-api-key.service repovec-provision.service",
        "Requires=network-online.target",
    )
}

fn api_key_after_dependency_wrong() -> String {
    qdrant_quadlet_contents()
        .replace("After=repovec-qdrant-api-key.service", "After=network-online.target")
}

/// The `Requires=` directive keeps the API-key service but loses the
/// provisioning oneshot, so the quadlet would start Qdrant against an
/// unprovisioned `/var/lib/repovec/qdrant-storage`.
fn provision_dependency_missing() -> String {
    qdrant_quadlet_contents().replace(
        "Requires=repovec-qdrant-api-key.service repovec-provision.service",
        "Requires=repovec-qdrant-api-key.service",
    )
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

/// One API-key violation: a mutated Quadlet, why it must be rejected, and the
/// typed error the validator must report for it.
struct ApiKeyCase {
    /// The mutated Quadlet text.
    contents: String,
    /// Why the contract rejects this Quadlet.
    reason: &'static str,
    /// The error the validator must return.
    expected: QdrantQuadletError,
}

/// Builds a case that must be rejected with `expected`.
fn case(contents: String, reason: &'static str, expected: QdrantQuadletError) -> ApiKeyCase {
    ApiKeyCase { contents, reason, expected }
}

#[rstest]
#[case::missing_api_key_secret(case(
    api_key_secret_missing(),
    "a Quadlet with no API-key secret would start Qdrant with no authentication",
    QdrantQuadletError::MissingApiKeySecret,
))]
#[case::incorrect_api_key_secret_name(case(
    api_key_secret_name_is_wrong(),
    "the secret must be the named appliance secret, not an arbitrary one",
    QdrantQuadletError::IncorrectApiKeySecret {
        secret: String::from("qdrant-key,type=env,target=QDRANT__SERVICE__API_KEY"),
    },
))]
#[case::incorrect_api_key_secret_target(case(
    api_key_secret_target_is_wrong(),
    "the secret must be delivered as the service API key, not the read-only key",
    QdrantQuadletError::IncorrectApiKeySecret {
        secret: String::from(
            "repovec-qdrant-api-key,type=env,target=QDRANT__SERVICE__READ_ONLY_API_KEY",
        ),
    },
))]
#[case::incorrect_api_key_secret_type(case(
    api_key_secret_type_is_wrong(),
    "the secret must be injected as an environment variable, not mounted",
    QdrantQuadletError::IncorrectApiKeySecret {
        secret: String::from("repovec-qdrant-api-key,type=mount,target=QDRANT__SERVICE__API_KEY"),
    },
))]
#[case::contradictory_api_key_secret_type(case(
    api_key_secret_type_is_contradictory(),
    "two type= fields cannot both apply, so the Quadlet is ambiguous",
    QdrantQuadletError::IncorrectApiKeySecret {
        secret: String::from(
            "repovec-qdrant-api-key,type=env,type=mount,target=QDRANT__SERVICE__API_KEY",
        ),
    },
))]
#[case::missing_api_key_requires_dependency(case(
    api_key_requires_dependency_missing(),
    "Requires= must pull the API-key oneshot in",
    QdrantQuadletError::MissingUnitDependency {
        directive: "Requires",
        expected: "repovec-qdrant-api-key.service",
    },
))]
#[case::missing_api_key_after_dependency(case(
    api_key_after_dependency_missing(),
    "After= must sequence the API-key oneshot first",
    QdrantQuadletError::MissingUnitDependency {
        directive: "After",
        expected: "repovec-qdrant-api-key.service",
    },
))]
#[case::incorrect_api_key_requires_dependency(case(
    api_key_requires_dependency_wrong(),
    "Requires= naming an unrelated unit leaves the API key unprovisioned",
    QdrantQuadletError::IncorrectUnitDependency {
        directive: "Requires",
        expected: "repovec-qdrant-api-key.service",
        actual: String::from("network-online.target"),
    },
))]
#[case::incorrect_api_key_after_dependency(case(
    api_key_after_dependency_wrong(),
    "After= naming an unrelated unit removes the ordering guarantee",
    QdrantQuadletError::IncorrectUnitDependency {
        directive: "After",
        expected: "repovec-qdrant-api-key.service",
        actual: String::from("network-online.target"),
    },
))]
#[case::incorrect_provision_dependency(case(
    provision_dependency_missing(),
    "Requires= must also name the provisioning oneshot, or Qdrant starts unprovisioned",
    QdrantQuadletError::IncorrectUnitDependency {
        directive: "Requires",
        expected: "repovec-provision.service",
        actual: String::from("repovec-qdrant-api-key.service"),
    },
))]
#[case::inline_api_key_environment(case(
    inline_api_key_environment(),
    "an inline API key leaks the secret into the unit file",
    QdrantQuadletError::InlineApiKeyEnvironmentDisallowed {
        environment: String::from("QDRANT__SERVICE__API_KEY=<redacted>"),
    },
))]
#[case::quoted_inline_api_key_environment(case(
    quoted_inline_api_key_environment(),
    "quoting the value does not make an inline API key acceptable",
    QdrantQuadletError::InlineApiKeyEnvironmentDisallowed {
        environment: String::from("QDRANT__SERVICE__API_KEY=<redacted>"),
    },
))]
#[case::multi_assignment_inline_api_key_environment(case(
    multi_assignment_inline_api_key_environment(),
    "an inline API key must still be found after the value is tokenized",
    QdrantQuadletError::InlineApiKeyEnvironmentDisallowed {
        environment: String::from("QDRANT__SERVICE__API_KEY=<redacted>"),
    },
))]
fn api_key_quadlet_violations_report_typed_errors(#[case] case: ApiKeyCase) {
    let error = validate_qdrant_quadlet(&case.contents, &()).expect_err(case.reason);

    assert_eq!(error, case.expected);
}

#[rstest]
#[case::missing_api_key_requires_dependency(
    api_key_requires_dependency_missing(),
    "missing_api_key_requires_dependency_display"
)]
#[case::incorrect_api_key_after_dependency(
    api_key_after_dependency_wrong(),
    "incorrect_api_key_after_dependency_display"
)]
#[case::incorrect_provision_dependency(
    provision_dependency_missing(),
    "incorrect_provision_dependency_display"
)]
#[case::missing_api_key_secret(api_key_secret_missing(), "missing_api_key_secret_display")]
#[case::incorrect_api_key_secret_type(
    api_key_secret_type_is_wrong(),
    "incorrect_api_key_secret_type_display"
)]
#[case::contradictory_api_key_secret_type(
    api_key_secret_type_is_contradictory(),
    "contradictory_api_key_secret_type_display"
)]
#[case::inline_api_key_environment(
    inline_api_key_environment(),
    "inline_api_key_environment_display"
)]
fn api_key_error_display_messages_remain_stable(
    #[case] contents: String,
    #[case] snapshot_label: &str,
) {
    let display = validate_qdrant_quadlet(&contents, &())
        .expect_err("invalid API-key quadlets should still return an error for the snapshot")
        .to_string();

    assert_snapshot!(snapshot_label, display);
}
