//! Unit tests for the traversal budgets.

use rstest::rstest;

use super::{
    DEFAULT_MAX_CONTINUATION_IDENTITY_BYTES,
    DEFAULT_MAX_LINK_HEADER_BYTES,
    DEFAULT_MAX_PAGE_BYTES,
    DEFAULT_MAX_PAGES,
    DEFAULT_MAX_RECORDS,
    DEFAULT_MAX_TOTAL_BYTES,
    ListingLimits,
};

#[test]
fn defaults_match_the_documented_budgets() {
    let limits = ListingLimits::defaults();

    assert_eq!(limits.max_pages(), DEFAULT_MAX_PAGES, "page budget changed");
    assert_eq!(
        limits.max_records(),
        DEFAULT_MAX_RECORDS,
        "record budget changed"
    );
    assert_eq!(
        limits.max_page_bytes(),
        DEFAULT_MAX_PAGE_BYTES,
        "page byte budget changed"
    );
    assert_eq!(
        limits.max_total_bytes(),
        DEFAULT_MAX_TOTAL_BYTES,
        "total byte budget changed"
    );
    assert_eq!(
        limits.max_link_header_bytes(),
        DEFAULT_MAX_LINK_HEADER_BYTES,
        "Link header budget changed"
    );
    assert_eq!(
        limits.max_continuation_identity_bytes(),
        DEFAULT_MAX_CONTINUATION_IDENTITY_BYTES,
        "continuation identity budget changed"
    );
}

#[test]
fn default_reports_the_same_budgets_as_defaults() {
    assert_eq!(
        ListingLimits::default(),
        ListingLimits::defaults(),
        "Default must agree with defaults"
    );
}

#[rstest]
#[case::pages(ListingLimits::defaults().with_max_pages(0))]
#[case::records(ListingLimits::defaults().with_max_records(0))]
#[case::page_bytes(ListingLimits::defaults().with_max_page_bytes(0))]
#[case::total_bytes(ListingLimits::defaults().with_max_total_bytes(0))]
#[case::link_header(ListingLimits::defaults().with_max_link_header_bytes(0))]
#[case::continuation(ListingLimits::defaults().with_max_continuation_identity_bytes(0))]
fn a_zero_budget_is_rejected(#[case] limits: ListingLimits) {
    let result = limits.validate();

    assert!(
        result.is_err(),
        "a budget admitting no work must be rejected, got {limits:?}"
    );
}

#[test]
fn a_single_unit_budget_is_accepted() {
    let limits = ListingLimits::defaults()
        .with_max_pages(1)
        .with_max_records(1)
        .with_max_page_bytes(1)
        .with_max_total_bytes(1)
        .with_max_link_header_bytes(1)
        .with_max_continuation_identity_bytes(1);

    assert_eq!(
        limits.validate().ok(),
        Some(limits),
        "positive budgets must validate unchanged"
    );
}
