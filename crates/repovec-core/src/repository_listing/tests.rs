//! Unit tests for the repository listing domain contract.

use rstest::rstest;

use super::{
    RateLimitInfo,
    RepositoryId,
    RepositoryListingError,
    RepositoryName,
    RepositoryOwner,
    RepositorySummary,
    ResourceBudget,
};
#[test]
fn repository_id_rejects_zero() {
    let error = RepositoryId::new(0).expect_err("zero must not be a repository identifier");

    assert_eq!(
        error,
        RepositoryListingError::InvalidInput {
            field: "repository id",
            reason: "must be a positive number",
        },
        "zero should be rejected as invalid input",
    );
}

#[test]
fn repository_id_preserves_positive_values() {
    let id = RepositoryId::new(u64::MAX).expect("a positive identifier should be accepted");

    assert_eq!(id.get(), u64::MAX, "the identifier should be preserved");
    assert_eq!(id.to_string(), "18446744073709551615");
}

#[rstest]
#[case("octocat")]
#[case("Acme-Corp")]
#[case("a")]
fn owner_accepts_github_logins(#[case] login: &str) {
    let owner = RepositoryOwner::new(login).expect("a non-empty login should be accepted");

    assert_eq!(owner.as_str(), login, "the login should be preserved");
}

#[rstest]
#[case("")]
#[case("bad\nlogin")]
#[case("bad\tlogin")]
#[case("bad\u{7f}login")]
fn owner_rejects_empty_and_control_bearing_logins(#[case] login: &str) {
    let result = RepositoryOwner::new(login);

    assert!(
        matches!(result, Err(RepositoryListingError::InvalidInput { .. })),
        "login {login:?} should be rejected",
    );
}

#[rstest]
#[case::empty("", "must not be empty")]
#[case::control_bearing("repo\r\nname", "must not contain control characters")]
fn repository_name_rejects_invalid_values(#[case] name: &str, #[case] reason: &'static str) {
    let error = RepositoryName::new(name).expect_err("an invalid repository name must be refused");

    assert_eq!(
        error,
        RepositoryListingError::InvalidInput {
            field: "repository name",
            reason,
        },
        "repository name {name:?} must be rejected with its documented reason",
    );
}

#[test]
fn summary_exposes_validated_components() {
    let repository = RepositorySummary::new(
        RepositoryId::new(11).expect("the identifier should be valid"),
        RepositoryOwner::new("acme").expect("the owner should be valid"),
        RepositoryName::new("widgets").expect("the name should be valid"),
    )
    .with_details(true, None);

    assert_eq!(repository.id().get(), 11, "the identifier should be kept");
    assert_eq!(repository.owner().as_str(), "acme");
    assert_eq!(repository.name().as_str(), "widgets");
    assert!(repository.is_private(), "the privacy flag should be kept");
    assert_eq!(repository.default_branch(), None);
    assert_eq!(repository.to_string(), "acme/widgets");
}

#[test]
fn summary_keeps_default_branch_when_reported() {
    let repository = RepositorySummary::new(
        RepositoryId::new(12).expect("the identifier should be valid"),
        RepositoryOwner::new("acme").expect("the owner should be valid"),
        RepositoryName::new("widgets").expect("the name should be valid"),
    )
    .with_details(false, Some("trunk".to_owned()));

    assert_eq!(repository.default_branch(), Some("trunk"));
    assert!(!repository.is_private());
}

#[rstest]
#[case(
    RepositoryListingError::AuthenticationRejected,
    "authentication_rejected"
)]
#[case(RepositoryListingError::Forbidden, "forbidden")]
#[case(RepositoryListingError::TransportFailure, "transport_failure")]
#[case(RepositoryListingError::CycleDetected, "cycle_detected")]
#[case(
    RepositoryListingError::UnexpectedStatus { status: 500 },
    "unexpected_status"
)]
#[case(
    RepositoryListingError::InvalidResponse { reason: "bad body" },
    "invalid_response"
)]
#[case(
    RepositoryListingError::InvalidContinuation { reason: "bad link" },
    "invalid_continuation"
)]
#[case(
    RepositoryListingError::RateLimited {
        retry: RateLimitInfo::default(),
    },
    "rate_limited"
)]
#[case(
    RepositoryListingError::ResourceLimitExceeded {
        budget: ResourceBudget::Pages,
        limit: 10,
    },
    "resource_limit_exceeded"
)]
#[case(
    RepositoryListingError::InvalidInput {
        field: "field",
        reason: "reason",
    },
    "invalid_input"
)]
fn every_error_exposes_a_stable_category(
    #[case] error: RepositoryListingError,
    #[case] expected: &str,
) {
    assert_eq!(
        error.category(),
        expected,
        "category for {error:?} should be stable",
    );
}

#[test]
fn rate_limit_metadata_reports_any_supplied_value() {
    assert!(!RateLimitInfo::default().has_retry_metadata());
    assert!(RateLimitInfo::new(Some(1), None).has_retry_metadata());
    assert!(RateLimitInfo::new(None, Some(1)).has_retry_metadata());
}
