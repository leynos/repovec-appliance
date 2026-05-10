//! Unit tests covering Qdrant API-key-specific Quadlet validation.

use rstest::rstest;

use super::{QdrantQuadletError, checked_in_qdrant_quadlet, validate_qdrant_quadlet};

fn qdrant_quadlet_contents() -> String {
    let mut contents = String::new();
    contents.push_str(checked_in_qdrant_quadlet());
    contents
}

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
