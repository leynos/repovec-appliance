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

/// The page an accepted continuation resolves to.
const PAGE_THREE: &str = "https://api.github.com/user/repos?visibility=all&per_page=100&page=3";

/// Page two, in the parameter order the listing sends.
const PAGE_TWO: &str = "https://api.github.com/user/repos?visibility=all&per_page=100&page=2";

/// Page two with its query parameters reordered.
const REORDERED_PAGE_TWO: &str =
    "https://api.github.com/user/repos?page=2&per_page=100&visibility=all";

/// A path-only next link whose fixed filters are reordered.
const REORDERED_NEXT: &str = r#"</user/repos?per_page=100&visibility=all&page=3>; rel="next""#;

/// The page the reordered next link resolves to, keeping that order.
const REORDERED_PAGE_THREE: &str =
    "https://api.github.com/user/repos?per_page=100&visibility=all&page=3";

/// An absolute next link naming page three.
const ABSOLUTE_NEXT: &str =
    r#"<https://api.github.com/user/repos?visibility=all&per_page=100&page=3>; rel="next""#;

/// A path-only next link naming page three.
const PATH_ONLY_NEXT: &str = r#"</user/repos?visibility=all&per_page=100&page=3>; rel="next""#;

/// A next link whose relation list also names last.
const NEXT_THEN_LAST: &str = r#"</user/repos?visibility=all&per_page=100&page=3>; rel="next last""#;

/// A next link whose relation type uses mixed case.
const MIXED_CASE_RELATION: &str = r#"</user/repos?visibility=all&per_page=100&page=3>; rel="NeXt""#;

/// A next link whose relation list and parameter name use mixed case.
const MIXED_CASE_EVERYWHERE: &str =
    r#"</user/repos?visibility=all&per_page=100&page=3>; Rel="NeXt LaSt""#;

/// A next link whose anchor parameter uses mixed case.
const MIXED_CASE_ANCHOR: &str = r##"</user/repos?page=2>; rel="next"; Anchor="#section""##;

/// A next link carrying an extra quoted parameter with a comma and a quote.
const QUOTED_NEXT: &str =
    r#"</user/repos?visibility=all&per_page=100&page=3>; rel="next"; title="a,b\"c""#;

/// The outcome selecting a continuation from a header must produce.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Resolution {
    /// The header offers no continuation.
    Complete,
    /// The continuation resolves to this target.
    Target(&'static str),
    /// Selection refuses the header with this reason.
    Rejected(&'static str),
}

#[rstest]
#[case::no_header(&[], Resolution::Complete)]
#[case::absolute_next(&[ABSOLUTE_NEXT], Resolution::Target(PAGE_THREE))]
#[case::path_only_next(&[PATH_ONLY_NEXT], Resolution::Target(PAGE_THREE))]
#[case::relation_list(&[NEXT_THEN_LAST], Resolution::Target(PAGE_THREE))]
#[case::mixed_case_relation(&[MIXED_CASE_RELATION], Resolution::Target(PAGE_THREE))]
#[case::mixed_case_relation_and_parameter(
    &[MIXED_CASE_EVERYWHERE],
    Resolution::Target(PAGE_THREE)
)]
#[case::quoted_parameter(&[QUOTED_NEXT], Resolution::Target(PAGE_THREE))]
#[case::reordered_filters(&[REORDERED_NEXT], Resolution::Target(REORDERED_PAGE_THREE))]
#[case::first_relation_parameter_only(
    &[r#"</user/repos?page=3>; rel="last"; rel="next""#],
    Resolution::Complete
)]
#[case::two_next_links_in_one_header(
    &[r#"</user/repos?page=2>; rel="next", </user/repos?page=3>; rel="next""#],
    Resolution::Rejected("more than one next link")
)]
#[case::two_next_links_across_headers(
    &[
        r#"</user/repos?page=2>; rel="next""#,
        r#"</user/repos?page=3>; rel="next""#,
    ],
    Resolution::Rejected("more than one next link")
)]
#[case::empty_list_element(
    &[r#"</user/repos?page=2>; rel="next", ,"#],
    Resolution::Rejected("Link header contains an empty list element")
)]
#[case::malformed_header(
    &["this is not a link header"],
    Resolution::Rejected("Link header is malformed")
)]
#[case::anchored_next(
    &[r##"</user/repos?page=2>; rel="next"; anchor="#section""##],
    Resolution::Rejected("an anchored next link has no listing context")
)]
#[case::mixed_case_anchor(
    &[MIXED_CASE_ANCHOR],
    Resolution::Rejected("an anchored next link has no listing context")
)]
fn a_next_continuation_is_selected_only_when_the_header_is_unambiguous(
    #[case] headers: &[&str],
    #[case] expected: Resolution,
) {
    let result = select(headers);

    match (result, expected) {
        (Ok(None), Resolution::Complete) => {}
        (Ok(Some(url)), Resolution::Target(target)) => {
            assert_eq!(
                url.as_str(),
                target,
                "the continuation must resolve to the reported page"
            );
        }
        (
            Err(RepositoryListingError::InvalidContinuation { reason }),
            Resolution::Rejected(cause),
        ) => {
            assert_eq!(reason, cause, "the refusal must name its cause");
        }
        (actual, wanted) => panic!("selection produced {actual:?}, expected {wanted:?}"),
    }
}

#[rstest]
#[case::other_origin("https://evil.example/user/repos?visibility=all&per_page=100&page=2")]
#[case::downgrade("http://api.github.com/user/repos?visibility=all&per_page=100&page=2")]
// The path is the only difference from `current`: the fixed query is intact,
// so removing the path check would leave this continuation accepted.
#[case::other_path(
    "https://api.github.com/installation/repositories?visibility=all&per_page=100&page=2"
)]
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

/// Which retention budget an oversized value crosses.
#[derive(Clone, Copy, Debug)]
enum RetentionProbe {
    /// A `Link` header value over its byte budget.
    LinkHeader,
    /// A retained continuation identity over its byte budget.
    Identity,
}

#[rstest]
#[case::link_header(RetentionProbe::LinkHeader, ResourceBudget::LinkHeaderBytes)]
#[case::identity(RetentionProbe::Identity, ResourceBudget::ContinuationIdentityBytes)]
fn an_oversized_value_is_a_limit_failure(
    #[case] probe: RetentionProbe,
    #[case] budget: ResourceBudget,
) {
    let initial = initial();
    let error = match probe {
        RetentionProbe::LinkHeader => {
            let context = ContinuationContext {
                initial: &initial,
                fixed_query: FIXED,
                max_header_bytes: 8,
                max_identity_bytes: 16 * 1024,
            };
            let header = headers(&[r#"</user/repos?page=2>; rel="next""#]);
            next_continuation(&header, &current(), &context).err()
        }
        RetentionProbe::Identity => {
            let context = ContinuationContext {
                initial: &initial,
                fixed_query: FIXED,
                max_header_bytes: 16 * 1024,
                max_identity_bytes: 8,
            };
            continuation_identity(&current(), &context).err()
        }
    };

    assert_eq!(
        error,
        Some(RepositoryListingError::ResourceLimitExceeded { budget, limit: 8 }),
        "an oversized {probe:?} must fail before it is retained"
    );
}

#[rstest]
#[case::reordered_query(PAGE_TWO, REORDERED_PAGE_TWO, true)]
#[case::different_pages(PAGE_TWO, PAGE_THREE, false)]
fn continuation_identity_compares_pages_not_query_order(
    #[case] left: &str,
    #[case] right: &str,
    #[case] is_same: bool,
) {
    let initial = initial();
    let context = context(&initial);
    let left_url = Url::parse(left).expect("the left identity URL should parse");
    let right_url = Url::parse(right).expect("the right identity URL should parse");
    let left_identity =
        continuation_identity(&left_url, &context).expect("the left identity should be calculable");
    let right_identity = continuation_identity(&right_url, &context)
        .expect("the right identity should be calculable");

    assert_eq!(
        left_identity == right_identity,
        is_same,
        "identity equality for {left} and {right} must be {is_same}"
    );
}
