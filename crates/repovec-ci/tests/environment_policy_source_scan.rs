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
//! Two further routes were found in review and measured on 2026-09-14, each on
//! a probe crate with the policy configured.
//!
//! An attribute whose body is a macro metavariable is decided by the call
//! site. A `macro_rules!` arm writing `#[$attr]` over a `std::env::var` call,
//! invoked as `forward!(allow(clippy::disallowed_methods))`, silenced that
//! call: Clippy reported the unforwarded call beside it and nothing about the
//! forwarded one, and `clippy::allow_attributes` said nothing about either.
//! `#[$attr]` does not parse as a `Meta` and the invocation carries no `#`, so
//! neither half is visible alone. The construction is refused rather than
//! resolved, and only where it could bear on the policy: a body beginning with
//! `$`, or with `allow`, `expect` or `cfg_attr`.
//!
//! `include!` resolves a path, not a module, and rustc parses the target as
//! Rust whatever its extension. `include!("policy.rs.txt")` compiled an
//! `#[allow(clippy::disallowed_methods, reason = "..")]` inside the target,
//! which silenced a `std::env::var` call there, while an enclosing
//! `#[expect(clippy::allow_attributes, reason = "..")]` kept the guard quiet.
//! The scan reads `.rs` files, so it never saw the target. An `include!` is
//! now a finding unless its target is a literal `.rs` path.
//! That target is parsed as one `syn::LitStr` and its decoded value judged,
//! never its spelling, which `r"support.rs"` and `"support\x2Ers"` show apart.
//!
//! Only a `macro_rules!` transcriber is walked, never an invocation's
//! arguments and never a matcher. `consume!(#[allow(clippy::style)])` hands an
//! attribute to a macro that discards it, and reporting that would be a false
//! positive. This does not reopen the route above: an attribute passed in as
//! an argument and then emitted must pass through a `#[$meta]` in the
//! definition, which is refused. An attribute synthesized by a procedural
//! macro remains out of reach, as it always was.
//!
//! Those three rules were mutation-proved in both directions on 2026-09-14,
//! each applied alone to `scan.rs` and run through the build:
//!
//! - naming an unprotected lint in the forwarded finding fails
//!   `a_suppression_is_an_offence::case_10_forwarded_from_a_macro_argument`;
//! - widening `is_forwarded` to every unparsable attribute body fails
//!   `a_construction_that_only_resembles_a_route_is_not_an_offence::
//!   case_2_forwarded_doc_and_derive`;
//! - treating every `include!` target as scanned fails
//!   `a_suppression_is_an_offence::case_11_included_from_an_unscanned_file`
//!   and `case_12_included_from_a_computed_path`;
//! - walking every macro's whole token stream, as this contract did before,
//!   fails the consumed-argument and matcher-only cases;
//! - walking a `macro_rules!` matcher as well as its transcriber fails the
//!   matcher-only case.
//!
//! The last three are the ones worth keeping. A contract that reports a false
//! positive gets switched off, so each rule is proved narrow as well as
//! sufficient.
//!
//! The `include!` rule was proved again on 2026-09-15, after review found that
//! judging a target's source spelling reported valid targets; those two
//! mutations are recorded on the rule itself in
//! `environment_policy_scan/tokens.rs`.
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
//! The contract is six files, to stay inside the 400-line limit that
//! Whitaker's `module_max_lines` enforces as well as the repository guide.
//! `environment_policy_scan/sources.rs` decides which files are read,
//! `environment_policy_scan/scan.rs` decides what they mean,
//! `environment_policy_scan/tokens.rs` recovers attributes from macro token
//! streams and holds the rules that keep that walk narrow,
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
#[path = "environment_policy_scan/tokens.rs"]
mod tokens;
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

/// Return `Ok` when `source` yields exactly one finding, naming `expected`.
///
/// The name is checked as well as the count, because a count alone accepts a
/// finding that reports the wrong lint, and these sample-based cases are where
/// a wrong name would otherwise go unnoticed.
fn one_offence(source: &str, expected: &str) -> Result<(), Failure> {
    let found = suppressed_lints(source)?;
    let names: Vec<&str> = found.iter().map(|(lint, _)| lint.as_str()).collect();
    ensure_that(
        names == [expected],
        format!("{source:?} must yield one finding naming {expected:?}, got {found:?}"),
    )
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
    one_offence(&format!("#![allow({lint}, reason = \"probe\")]\n"), lint)
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
/// - `forwarded_from_a_macro_argument`: `#[$attr]` names no lint and does not
///   parse as a `Meta`, while the invocation carries no `#`, so neither half is
///   visible on its own. The finding names no lint either, because none is
///   written.
/// - `included_from_an_unscanned_file`: `include!` resolves a path, not a
///   module, and rustc parses the target as Rust whatever its extension, so a
///   suppression can sit in a file the scan never reads.
/// - `included_from_a_computed_path`: a target assembled by `concat!` and `env!`
///   cannot be resolved here. Only one string literal is accepted, and this is
///   the case that says so.
#[rstest]
#[case::nested_in_cfg_attr(
    "#![cfg_attr(all(), allow(clippy::disallowed_methods, reason = \"x\"))]\n",
    "clippy::disallowed_methods"
)]
#[case::lint_group("#![allow(clippy::style)]\n", "clippy::style")]
#[case::spacing_and_a_parenthesis(
    "#![allow (warnings, reason = \"see the note (below)\")]\n",
    "warnings"
)]
#[case::from_a_macro_arm(
    include_str!("fixtures/env_policy_samples/macro_arm.rs.txt"),
    "clippy::disallowed_methods"
)]
#[case::two_macro_arms_deep(
    include_str!("fixtures/env_policy_samples/two_macro_arms_deep.rs.txt"),
    "clippy::style"
)]
#[case::crate_scoped_expect(
    "#![expect(clippy::disallowed_methods, reason = \"x\")]\n",
    "clippy::disallowed_methods"
)]
#[case::crate_scoped_expect_in_cfg_attr(
    "#![cfg_attr(all(), expect(clippy::disallowed_methods, reason = \"x\"))]\n",
    "clippy::disallowed_methods"
)]
#[case::raw_attribute("#![r#allow(clippy::disallowed_methods)]\n", "clippy::disallowed_methods")]
#[case::raw_lint("#![allow(clippy::r#style)]\n", "clippy::style")]
#[case::forwarded_from_a_macro_argument(
    include_str!("fixtures/env_policy_samples/forwarded_attribute.rs.txt"),
    "whatever the call site passes"
)]
#[case::included_from_an_unscanned_file(
    include_str!("fixtures/env_policy_samples/included_unscanned_file.rs.txt"),
    "code from a file the scan does not read"
)]
#[case::included_from_a_computed_path(
    include_str!("fixtures/env_policy_samples/included_from_a_computed_path.rs.txt"),
    "code from a file the scan does not read"
)]
fn a_suppression_is_an_offence(
    #[case] source: &str,
    #[case] expected: &str,
) -> Result<(), Failure> {
    // One invariant, twelve spellings; the case labels say which is which.
    one_offence(source, expected)
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

/// Scenario: constructions that resemble a route without being one.
///
/// Invariant: none is a finding. Each is the false positive the corresponding
/// rule would produce if it were drawn one step wider, and a contract that
/// reports a false positive gets switched off.
///
/// - `consumed_macro_argument`: `consume!(#[allow(clippy::style)])` hands an
///   attribute to a macro that discards it. Only a `macro_rules!` transcriber
///   is scanned, never an invocation's arguments and never a matcher, because
///   nothing in either is necessarily written out.
/// - `forwarded_doc_and_derive`: `#[doc = $doc]` and `#[derive($trait)]` fail to
///   parse as a `Meta` exactly as `#[$attr]` does, and forwarding attributes is
///   ordinary in code-generating macros.
/// - `included_scanned_file`, `included_scanned_raw_literal` and
///   `included_scanned_escaped_literal`: each names a literal `.rs` path the
///   scan already reads. The last two are spelled `r"support.rs"` and
///   `"support\x2Ers"`, whose decoded value ends in `.rs` while their spelling
///   does not.
/// - `matcher_only_attribute`: a `macro_rules!` arm matches
///   `#[allow(clippy::style)]` and its transcriber emits only `$item`, so the
///   attribute is consumed rather than written out.
#[rstest]
#[case::consumed_macro_argument(include_str!(
    "fixtures/env_policy_samples/consumed_macro_argument.rs.txt"
))]
#[case::forwarded_doc_and_derive(include_str!(
    "fixtures/env_policy_samples/forwarded_doc_and_derive.rs.txt"
))]
#[case::included_scanned_file(include_str!(
    "fixtures/env_policy_samples/included_scanned_file.rs.txt"
))]
#[case::included_scanned_raw_literal(include_str!(
    "fixtures/env_policy_samples/included_scanned_raw_literal.rs.txt"
))]
#[case::included_scanned_escaped_literal(include_str!(
    "fixtures/env_policy_samples/included_scanned_escaped_literal.rs.txt"
))]
#[case::matcher_only_attribute(include_str!(
    "fixtures/env_policy_samples/matcher_only_attribute.rs.txt"
))]
fn a_construction_that_only_resembles_a_route_is_not_an_offence(
    #[case] source: &str,
) -> Result<(), Failure> {
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
