//! Source scan closing the inner-attribute route around the policy.
//!
//! The other two contracts check the configuration and prove the lint
//! fires. Neither sees a source file that switches the lint off. A
//! crate-level inner attribute does exactly that:
//!
//! ```ignore
//! #![allow(clippy::disallowed_methods, reason = "...")]
//! ```
//!
//! `clippy::allow_attributes`, which the workspace denies, does not fire
//! on inner attributes, so the whole crate stops enforcing the policy
//! with every gate still green. Measured on `main` at 5596f65: with that
//! attribute in `repovec-ci`'s crate root and an unannotated
//! `std::env::var("HOME")` beside it, `make lint` exited 0, the
//! configuration contract passed all five tests, the Clippy fixture test
//! passed, and the workflow contracts passed all fifteen.
//!
//! `expect` is judged by scope, not exempted outright. An item-scoped
//! `#[expect(..., reason = "...")]` is the sanctioned form, and it is
//! safe because it warns as soon as its one item stops needing it. A
//! crate-scoped `#![expect(...)]` has no such property: any single call
//! beneath it fulfils it, so it neither reports the call nor warns that
//! it went unfulfilled. Measured on this branch, it produced no
//! diagnostic of either kind. At crate scope `expect` is an `allow` that
//! looks responsible.
//!
//! A macro arm is the one place parsing is not enough. `syn` keeps a
//! `macro_rules!` body as opaque tokens, so an `allow` emitted from an
//! arm never reaches `visit_attribute` while Clippy expands and honours
//! it; measured on this branch, such an arm over a `std::env::var` call
//! produced no diagnostic. Macro token streams are therefore walked
//! alongside the parsed attributes, under the same judgement. A string
//! literal is a single token, so the property that made parsing worth
//! having survives the drop to tokens.
//!
//! The file is parsed rather than searched. Review found three shapes a
//! text scan could not handle and two further routes around the policy,
//! all confirmed against Clippy before being fixed: a suppression nested
//! in `#![cfg_attr(all(), allow(...))]`, and a suppression naming the
//! lint's group rather than the lint, since Clippy places
//! `disallowed_methods` in `style`. Both suppress the lint with zero
//! diagnostics.
//!
//! Mutation proof, recorded 2026-09-08; each applied alone to a real
//! source file, run through the build, and reverted:
//!
//! - `#![allow(clippy::disallowed_methods)]` in a crate root fails
//!   `no_source_file_allows_a_policy_lint`;
//! - `#![allow(clippy::style)]`, naming the group rather than the lint,
//!   fails it;
//! - `#![cfg_attr(all(), allow(clippy::disallowed_methods))]` fails it;
//! - `#![allow(clippy::all)]` spread over several lines fails it;
//! - `#[allow(warnings)]` on an item fails it;
//! - `#![allow(clippy::restriction)]`, which silences the guard lint
//!   rather than the policy lint, fails it;
//! - an `allow` emitted from a `macro_rules!` arm fails it, and so does
//!   one two macro definitions deep;
//! - a crate-scoped `#![expect(clippy::disallowed_methods)]` fails it,
//!   as does the same reached through an inner `cfg_attr`;
//! - the raw spellings `#![r#allow(...)]` and `clippy::r#style` fail it,
//!   both being spellings Clippy honours;
//! - an item-scoped `#[expect(..., reason = "...")]` must and does keep
//!   passing, since it is the form the taxonomy sanctions;
//! - `#[allow(clippy::alloc_instead_of_core)]` must and does keep
//!   passing, as does attribute-shaped text inside a macro argument.
//!
//! That last case is the one that keeps this contract honest. Its name
//! begins with `clippy::all`, so an earlier draft comparing lint names
//! by substring would have reported it. Names are compared as paths.
//!
//! Each of those routes is a place the enforcement mechanism could not
//! see, and a table of samples cannot say what the scan does with a lint
//! name, a nesting depth or a reason string nobody wrote down.
//! `environment_policy_scan/properties.rs` states the claim itself over
//! generated inputs: what the scan reports is decided by the lint named
//! and by the scope the attribute takes, and by nothing else. Three
//! further mutations, each applied alone to `scan.rs` and run through
//! the build, are recorded there.
//!
//! The contract is five files, to stay inside the 400-line limit.
//! `environment_policy_scan/sources.rs` decides which files are read,
//! `environment_policy_scan/scan.rs` decides what they mean,
//! `environment_policy_scan/workspace.rs` holds the contracts over this
//! repository's own sources and over the scan's error paths,
//! `environment_policy_scan/properties.rs` holds the properties, and the
//! judgements over samples stay here. The samples live in
//! `tests/fixtures/env_policy_samples` as `.rs.txt` files: a `.rs` file
//! there would be read by the workspace scan itself and reported as an
//! offence.

// An integration test's crate root resolves `mod` against `tests/`, not against
// a directory of its own, so each module names its path explicitly. The
// directory is `environment_policy_scan` rather than the test file's own name
// because `clippy::self_named_module_files` rejects a `foo.rs` beside a `foo/`.
// A subdirectory of `tests/` is not built as a test binary of its own, which is
// what keeps this contract in one binary.
#[path = "environment_policy_scan/properties.rs"]
mod properties;
#[path = "environment_policy_scan/scan.rs"]
mod scan;
#[path = "environment_policy_scan/sources.rs"]
mod sources;
#[path = "environment_policy_scan/workspace.rs"]
mod workspace;

use rstest::rstest;
use scan::suppressed_lints;

/// Error type carried by the helpers and the tests below.
///
/// The helpers report through it rather than panicking, because
/// `allow-expect-in-tests` does not reach a function outside a `#[test]`
/// body, and `clippy::panic_in_result_fn` is denied besides.
type Failure = Box<dyn std::error::Error>;

/// Return `Err` carrying `message` when `condition` does not hold.
fn ensure_that(condition: bool, message: String) -> Result<(), Failure> {
    if condition { Ok(()) } else { Err(message.into()) }
}

/// Return `Ok` when `source` yields exactly one finding.
fn one_offence(source: &str) -> Result<(), Failure> {
    let found = suppressed_lints(source)?;
    ensure_that(found.len() == 1, format!("{source:?} must yield one finding, got {found:?}"))
}

/// Return `Ok` when `source` yields no finding.
fn no_offence(source: &str) -> Result<(), Failure> {
    let found = suppressed_lints(source)?;
    ensure_that(found.is_empty(), format!("{source:?} must yield no finding, got {found:?}"))
}

/// Scenario: each lint in the protected set is suppressed at crate scope.
///
/// Invariant: every one of them is reported. The set is not a list of names
/// that look related: each was confirmed against Clippy to switch the policy
/// off, upwards through the groups that contain `disallowed_methods` and
/// sideways through the guard lints that make "`expect`, never `allow`"
/// enforceable. A name that stops being reported is a name that stopped being
/// protected, and this says which one.
#[rstest]
#[case::policy_lint("clippy::disallowed_methods")]
#[case::policy_lints_group("clippy::style")]
#[case::all("clippy::all")]
#[case::warnings("warnings")]
#[case::guard_lint("clippy::allow_attributes")]
#[case::guard_lint_without_reason("clippy::allow_attributes_without_reason")]
#[case::guard_lints_group("clippy::restriction")]
fn every_protected_lint_is_reported(#[case] lint: &str) -> Result<(), Failure> {
    one_offence(&format!("#![allow({lint}, reason = \"probe\")]\n"))
}

/// Scenario: the same suppression written every way Clippy honours.
///
/// Invariant: each is reported. These are one invariant, not nine: a
/// suppression that reaches past the item it is written on is an offence
/// however it is spelled. Each case is a spelling review or measurement found
/// the mechanism could not see, and each is labelled so a spelling that stops
/// being caught is named in the failure rather than hidden behind an earlier
/// one.
///
/// - `nested_in_cfg_attr`: Clippy honours the nested `allow` and
///   `clippy::allow_attributes` does not report it, so a scan looking for a
///   line beginning `#![allow(` misses it. The condition is not evaluated.
/// - `lint_group`: Clippy places `disallowed_methods` in `style`, so the group
///   switches the policy off while never naming the lint.
/// - `spacing_and_a_parenthesis`: a fixed opener misses the spaced form, and
///   counting raw parentheses ends the attribute early at the one inside the
///   reason string.
/// - `from_a_macro_arm` and `two_macro_arms_deep`: `syn` keeps an arm's body an
///   opaque token stream, so the attribute never reaches `visit_attribute`,
///   while Clippy expands and honours it.
/// - `crate_scoped_expect` and `crate_scoped_expect_in_cfg_attr`: at crate
///   scope `expect` behaves as an `allow`, since any one call beneath it
///   fulfils it, and the scope belongs to the outermost attribute.
/// - `raw_attribute` and `raw_lint`: Clippy honours `r#allow` and
///   `clippy::r#style` exactly as it honours the plain spellings.
#[rstest]
#[case::nested_in_cfg_attr(
    "#![cfg_attr(all(), allow(clippy::disallowed_methods, reason = \"x\"))]\n"
)]
#[case::lint_group("#![allow(clippy::style)]\n")]
#[case::spacing_and_a_parenthesis("#![allow (warnings, reason = \"see the note (below)\")]\n")]
#[case::from_a_macro_arm(include_str!("fixtures/env_policy_samples/macro_arm.rs.txt"))]
#[case::two_macro_arms_deep(include_str!(
    "fixtures/env_policy_samples/two_macro_arms_deep.rs.txt"
))]
#[case::crate_scoped_expect("#![expect(clippy::disallowed_methods, reason = \"x\")]\n")]
#[case::crate_scoped_expect_in_cfg_attr(
    "#![cfg_attr(all(), expect(clippy::disallowed_methods, reason = \"x\"))]\n"
)]
#[case::raw_attribute("#![r#allow(clippy::disallowed_methods)]\n")]
#[case::raw_lint("#![allow(clippy::r#style)]\n")]
fn a_suppression_is_an_offence(#[case] source: &str) -> Result<(), Failure> {
    // One invariant, nine spellings; the case labels say which is which.
    one_offence(source)
}

/// Scenario: an item-scoped `expect` at a sanctioned composition root.
///
/// Invariant: the scan does not reject it. This is the sanctioned form, and it
/// is safe for the reason the crate-scoped one is not: it is attached to one
/// item, so it warns as soon as that item stops needing it. Rejecting it would
/// push contributors towards `allow`, which never warns at any scope.
#[test]
fn a_sanctioned_expect_is_not_an_offence() -> Result<(), Failure> {
    no_offence(include_str!("fixtures/env_policy_samples/sanctioned_item_expect.rs.txt"))
}

/// Scenario: attribute-shaped text in a string literal, a macro argument and a
/// doc comment.
///
/// Invariant: none is an offence. This is the false positive a text scan cannot
/// avoid, and a contract that reports one gets disabled. A string literal is a
/// single token, so the property parsing gave us survives the walk's drop to
/// tokens; the macro-argument case is the one that says so.
#[rstest]
#[case::string_literal(include_str!(
    "fixtures/env_policy_samples/text_in_a_string_literal.rs.txt"
))]
#[case::macro_argument(include_str!(
    "fixtures/env_policy_samples/text_in_a_macro_argument.rs.txt"
))]
#[case::prose(include_str!("fixtures/env_policy_samples/text_in_prose.rs.txt"))]
fn attribute_shaped_text_is_not_an_attribute(#[case] source: &str) -> Result<(), Failure> {
    no_offence(source)
}

/// Scenario: a lint whose name merely begins with a protected one.
///
/// Invariant: neither is reported. `clippy::all` is a prefix of
/// `clippy::alloc_instead_of_core`, and `clippy::allow_attributes` of
/// `clippy::allow_attributes_deprecated`. Lint names are compared as paths, not
/// as text: a substring test would reject both, and a contributor would have no
/// way to tell a real finding from a false one.
#[rstest]
#[case::beginning_with_clippy_all("clippy::alloc_instead_of_core")]
#[case::beginning_with_a_guard_lint("clippy::allow_attributes_deprecated")]
fn a_longer_lint_name_beginning_with_a_protected_one_is_not_an_offence(
    #[case] lint: &str,
) -> Result<(), Failure> {
    no_offence(&format!("#[allow({lint})]\nfn documented() {{}}\n"))
}
