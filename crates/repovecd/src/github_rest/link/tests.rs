//! Unit tests for continuation selection and confinement.
//!
//! These tests pin the behaviour the traversal relies on, so an upgrade of the
//! Link grammar crate that changes it fails here rather than in production.

use repovec_core::repository_listing::{RepositoryListingError, ResourceBudget};
use reqwest::{Url, header::HeaderValue};
use rstest::rstest;

use super::{ContinuationContext, continuation_identity, next_continuation};

/// Query pairs fixed for the listing the tests page.
const FIXED: &[(&str, &str)] = &[("visibility", "all"), ("per_page", "100")];

/// The first request of the listing the tests page.
fn initial() -> Url {
    Url::parse("https://api.github.com/user/repos?visibility=all&per_page=100")
        .expect("the initial test URL should parse")
}

/// The request a continuation is resolved against.
fn current() -> Url {
    Url::parse("https://api.github.com/user/repos?visibility=all&page=2&per_page=100")
        .expect("the current test URL should parse")
}

/// Builds the context used by every test in this module.
fn context(initial: &Url) -> ContinuationContext<'_> {
    ContinuationContext {
        initial,
        fixed_query: FIXED,
        max_header_bytes: 16 * 1024,
        max_identity_bytes: 16 * 1024,
    }
}

/// Builds header values from link header text.
fn headers(values: &[&str]) -> Vec<HeaderValue> {
    values
        .iter()
        .map(|value| HeaderValue::from_str(value).expect("test header text should be valid"))
        .collect()
}

/// Runs selection against the standard context.
fn select(values: &[&str]) -> Result<Option<Url>, RepositoryListingError> {
    let initial = initial();
    next_continuation(&headers(values), &current(), &context(&initial))
}

#[test]
fn a_missing_link_header_completes_the_listing() {
    let result = select(&[]);

    assert_eq!(
        result.ok(),
        Some(None),
        "no header must mean no continuation"
    );
}

#[test]
fn a_relative_next_link_resolves_against_the_current_request() {
    let next = select(&[
        r#"<https://api.github.com/user/repos?visibility=all&per_page=100&page=3>; rel="next""#,
    ])
    .expect("an absolute next link should be accepted")
    .expect("a next link should be present");

    assert_eq!(
        next.as_str(),
        "https://api.github.com/user/repos?visibility=all&per_page=100&page=3",
        "the continuation should be the reported target"
    );
}

#[test]
fn a_path_only_next_link_keeps_the_current_query() {
    let next = select(&[r#"</user/repos?visibility=all&per_page=100&page=3>; rel="next""#])
        .expect("a relative next link should be accepted")
        .expect("a next link should be present");

    assert_eq!(
        next.as_str(),
        "https://api.github.com/user/repos?visibility=all&per_page=100&page=3",
        "resolution should use the listing origin"
    );
}

#[test]
fn a_relation_list_containing_next_is_accepted() {
    let next = select(&[r#"</user/repos?visibility=all&per_page=100&page=3>; rel="next last""#])
        .expect("a relation list naming next should be accepted");

    assert!(next.is_some(), "a next relation in a list must be followed");
}

#[test]
fn only_the_first_relation_parameter_is_read() {
    let result = select(&[r#"</user/repos?page=3>; rel="last"; rel="next""#]);

    assert_eq!(
        result.ok(),
        Some(None),
        "only the first rel parameter decides the relation type"
    );
}

#[test]
fn a_quoted_parameter_may_contain_a_comma_and_an_escaped_quote() {
    let next = select(&[
        r#"</user/repos?visibility=all&per_page=100&page=3>; rel="next"; title="a,b\"c""#,
    ])
    .expect("a quoted parameter should not break parsing");

    assert!(
        next.is_some(),
        "the next link must survive quoted parameters"
    );
}

#[test]
fn two_next_links_in_one_header_are_rejected() {
    let result = select(&[r#"</user/repos?page=2>; rel="next", </user/repos?page=3>; rel="next""#]);

    assert_eq!(
        result.err(),
        Some(RepositoryListingError::InvalidContinuation {
            reason: "more than one next link",
        }),
        "an ambiguous continuation must be rejected rather than chosen"
    );
}

#[test]
fn two_next_links_across_headers_are_rejected() {
    let result = select(&[
        r#"</user/repos?page=2>; rel="next""#,
        r#"</user/repos?page=3>; rel="next""#,
    ]);

    assert!(
        matches!(
            result,
            Err(RepositoryListingError::InvalidContinuation { .. })
        ),
        "two headers naming different continuations are still ambiguous"
    );
}

#[test]
fn an_empty_list_element_is_rejected() {
    let result = select(&[r#"</user/repos?page=2>; rel="next", ,"#]);

    assert!(
        matches!(
            result,
            Err(RepositoryListingError::InvalidContinuation { .. })
        ),
        "an empty list element must be refused, not skipped"
    );
}

#[test]
fn a_malformed_header_is_rejected() {
    let result = select(&["this is not a link header"]);

    assert!(
        matches!(
            result,
            Err(RepositoryListingError::InvalidContinuation { .. })
        ),
        "malformed link text must be refused"
    );
}

#[test]
fn an_anchored_next_link_is_rejected() {
    let result = select(&[r##"</user/repos?page=2>; rel="next"; anchor="#section""##]);

    assert!(
        matches!(
            result,
            Err(RepositoryListingError::InvalidContinuation { .. })
        ),
        "an anchored next link has no listing context to follow"
    );
}

#[rstest]
#[case::other_origin("https://evil.example/user/repos?visibility=all&per_page=100&page=2")]
#[case::downgrade("http://api.github.com/user/repos?visibility=all&per_page=100&page=2")]
#[case::other_path("https://api.github.com/installation/repositories?per_page=100")]
#[case::userinfo("https://user:secret@api.github.com/user/repos?visibility=all&per_page=100")]
#[case::fragment("https://api.github.com/user/repos?visibility=all&per_page=100&page=2#x")]
#[case::added_filter(
    "https://api.github.com/user/repos?visibility=all&per_page=100&page=2&type=all"
)]
#[case::changed_filter("https://api.github.com/user/repos?visibility=private&per_page=100&page=2")]
#[case::dropped_filter("https://api.github.com/user/repos?per_page=100&page=2")]
#[case::repeated_page(
    "https://api.github.com/user/repos?visibility=all&per_page=100&page=2&page=3"
)]
#[case::other_port("https://api.github.com:8443/user/repos?visibility=all&per_page=100&page=2")]
fn a_continuation_outside_the_listing_is_rejected(#[case] target: &str) {
    let header = format!("<{target}>; rel=\"next\"");

    let result = select(&[header.as_str()]);

    assert!(
        matches!(
            result,
            Err(RepositoryListingError::InvalidContinuation { .. })
        ),
        "a continuation must stay inside the listing: {target}"
    );
}

#[test]
fn reordered_fixed_filters_are_still_confined() {
    let next = select(&[r#"</user/repos?per_page=100&visibility=all&page=3>; rel="next""#])
        .expect("query order must not decide confinement");

    assert!(
        next.is_some(),
        "order alone must not be treated as a change"
    );
}

#[test]
fn an_oversized_link_header_is_a_limit_failure() {
    let initial = initial();
    let context = ContinuationContext {
        initial: &initial,
        fixed_query: FIXED,
        max_header_bytes: 8,
        max_identity_bytes: 16 * 1024,
    };
    let header = headers(&[r#"</user/repos?page=2>; rel="next""#]);

    let result = next_continuation(&header, &current(), &context);

    assert_eq!(
        result.err(),
        Some(RepositoryListingError::ResourceLimitExceeded {
            budget: ResourceBudget::LinkHeaderBytes,
            limit: 8,
        }),
        "an oversized header must fail before it is parsed"
    );
}

#[test]
fn continuation_identity_ignores_query_order() {
    let initial = initial();
    let context = context(&initial);
    let first = Url::parse("https://api.github.com/user/repos?visibility=all&per_page=100&page=2")
        .expect("the first identity URL should parse");
    let second = Url::parse("https://api.github.com/user/repos?page=2&per_page=100&visibility=all")
        .expect("the second identity URL should parse");

    assert_eq!(
        continuation_identity(&first, &context).ok(),
        continuation_identity(&second, &context).ok(),
        "reordered queries must share one identity"
    );
}

#[test]
fn continuation_identity_distinguishes_different_pages() {
    let initial = initial();
    let context = context(&initial);
    let second = Url::parse("https://api.github.com/user/repos?visibility=all&per_page=100&page=2")
        .expect("the second identity URL should parse");
    let third = Url::parse("https://api.github.com/user/repos?visibility=all&per_page=100&page=3")
        .expect("the third identity URL should parse");

    assert_ne!(
        continuation_identity(&second, &context).ok(),
        continuation_identity(&third, &context).ok(),
        "distinct pages must have distinct identities"
    );
}

#[test]
fn an_oversized_continuation_identity_is_a_limit_failure() {
    let initial = initial();
    let context = ContinuationContext {
        initial: &initial,
        fixed_query: FIXED,
        max_header_bytes: 16 * 1024,
        max_identity_bytes: 8,
    };

    let result = continuation_identity(&current(), &context);

    assert_eq!(
        result.err(),
        Some(RepositoryListingError::ResourceLimitExceeded {
            budget: ResourceBudget::ContinuationIdentityBytes,
            limit: 8,
        }),
        "a retained identity must stay inside its budget"
    );
}
