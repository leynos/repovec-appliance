//! Property tests for the environment-policy source scan.
//!
//! The judgement tests beside these name the routes review and measurement
//! found: `cfg_attr`, lint groups, macro arms, raw identifiers, crate-scoped
//! `expect`. Each is one sample of a wider claim, and every route found so far
//! was a place the enforcement mechanism could not see. A sample cannot say
//! what the scan does with a lint name, a nesting depth or a reason string
//! nobody thought to write down.
//!
//! These state the claim itself over generated inputs: what the scan reports is
//! decided by the lint named and by the scope the attribute takes, and by
//! nothing else. Reason strings are generated with spaces, commas and
//! parentheses, because a parenthesis inside a reason defeated the text scan
//! this contract replaced, and macro nesting is generated to a depth of three
//! because the walk recurses through every token group.
//!
//! Mutation-proved on 2026-09-14, each applied alone to `scan.rs` and run
//! through the build:
//!
//! - dropping the `"expect" if scope == Scope::Crate` arm from
//!   `suppressed_by_meta` fails
//!   `a_protected_lint_is_reported_in_every_offending_form`;
//! - collecting an empty token stream in place of each macro body, so every
//!   helper stays used and the mutant compiles, fails it too;
//! - comparing lint names with `contains` rather than by path fails
//!   `an_unprotected_lint_is_never_reported`, on the two generated names that
//!   begin with the text of a protected one.

use proptest::prelude::*;

use crate::scan::{PROTECTED_LINTS, suppressed_lints};

/// Lints outside the protected set.
///
/// `clippy::alloc_instead_of_core` and `clippy::allow_attributes_deprecated`
/// each begin with the text of a protected name, so a scan comparing
/// substrings rather than paths would report both.
const UNPROTECTED_LINTS: [&str; 5] = [
    "dead_code",
    "unused_variables",
    "clippy::alloc_instead_of_core",
    "clippy::allow_attributes_deprecated",
    "clippy::pedantic",
];

/// A way of writing a suppression the policy refuses.
///
/// Most of these reach past the item they are written on. `ItemAllow` does
/// not: it lowers the deny for exactly one item, and is refused because the
/// policy's rule is `expect`, never `allow`, so that a suppression warns once
/// its site stops needing it.
#[derive(Debug, Clone, Copy)]
enum OffendingForm {
    /// A crate-level `allow`, which `clippy::allow_attributes` cannot see.
    InnerAllow,
    /// An item-level `allow`, which lowers the deny for that item.
    ItemAllow,
    /// A crate-level `expect`, fulfilled crate-wide by any one call.
    CrateExpect,
    /// An `allow` nested in a `cfg_attr`, which Clippy honours.
    NestedInCfgAttr,
    /// A crate-level `allow` with a space before the parenthesis.
    SpacedInnerAllow,
    /// A crate-level `allow` spelt with raw identifiers.
    RawInnerAllow,
}

/// Spell a lint path with every segment as a raw identifier.
fn as_raw(lint: &str) -> String {
    lint.split("::").map(|segment| format!("r#{segment}")).collect::<Vec<_>>().join("::")
}

impl OffendingForm {
    /// Render this form as a source suppressing `lint`.
    fn render(self, lint: &str, reason: &str) -> String {
        match self {
            Self::InnerAllow => format!("#![allow({lint}, reason = \"{reason}\")]\n"),
            Self::ItemAllow => {
                format!("#[allow({lint}, reason = \"{reason}\")]\nfn probe() {{}}\n")
            }
            Self::CrateExpect => format!("#![expect({lint}, reason = \"{reason}\")]\n"),
            Self::NestedInCfgAttr => {
                format!("#![cfg_attr(all(), allow({lint}, reason = \"{reason}\"))]\n")
            }
            Self::SpacedInnerAllow => format!("#![allow ({lint}, reason = \"{reason}\")]\n"),
            Self::RawInnerAllow => {
                format!("#![r#allow({}, reason = \"{reason}\")]\n", as_raw(lint))
            }
        }
    }
}

/// Every offending form, for a strategy to select from.
const OFFENDING_FORMS: [OffendingForm; 6] = [
    OffendingForm::InnerAllow,
    OffendingForm::ItemAllow,
    OffendingForm::CrateExpect,
    OffendingForm::NestedInCfgAttr,
    OffendingForm::SpacedInnerAllow,
    OffendingForm::RawInnerAllow,
];

/// Wrap `body` in `depth` nested `macro_rules!` definitions.
///
/// At depth zero the body is returned as written, which is the case the
/// parsed walk handles; beyond that it is reachable only through the
/// token walk.
fn in_macro_arms(body: &str, depth: usize) -> String {
    (0..depth).fold(body.to_owned(), |inner, level| {
        format!("macro_rules! level_{level} {{\n    () => {{\n{inner}    }};\n}}\n")
    })
}

/// Run the scan, turning a parse failure into a test-case failure.
fn scan(source: &str) -> Result<Vec<(String, String)>, TestCaseError> {
    suppressed_lints(source)
        .map_err(|error| TestCaseError::fail(format!("{source:?} should parse as Rust: {error}")))
}

/// A lint name the policy protects.
fn protected_lint() -> impl Strategy<Value = &'static str> {
    proptest::sample::select(PROTECTED_LINTS.to_vec())
}

/// A lint name the policy does not protect.
fn unprotected_lint() -> impl Strategy<Value = &'static str> {
    proptest::sample::select(UNPROTECTED_LINTS.to_vec())
}

/// Any lint name, protected or not.
fn any_lint() -> impl Strategy<Value = &'static str> {
    prop_oneof![protected_lint(), unprotected_lint()]
}

/// A way of writing a suppression the policy refuses.
fn offending_form() -> impl Strategy<Value = OffendingForm> {
    proptest::sample::select(OFFENDING_FORMS.to_vec())
}

/// A reason string, with the punctuation that defeats a text scan.
fn reason() -> impl Strategy<Value = String> { "[a-z ,()]{0,24}".prop_map(String::from) }

proptest! {
    /// Scenario: a protected lint is suppressed in any form the policy
    /// refuses, at any macro nesting depth up to three.
    ///
    /// Invariant: the scan reports exactly that lint, once. This is the claim
    /// the sample-based tests each pin one corner of, stated over all seven
    /// protected names rather than the handful the samples spell out.
    #[test]
    fn a_protected_lint_is_reported_in_every_offending_form(
        lint in protected_lint(),
        form in offending_form(),
        reason in reason(),
        depth in 0_usize..=3,
    ) {
        let source = in_macro_arms(&form.render(lint, &reason), depth);
        let found = scan(&source)?;
        let names: Vec<&str> = found.iter().map(|(name, _)| name.as_str()).collect();
        prop_assert_eq!(names, vec![lint], "source {:?}", source);
    }

    /// Scenario: a lint outside the protected set is suppressed in the same
    /// forms and at the same depths.
    ///
    /// Invariant: nothing is reported. A contract firing on every generated
    /// `#[allow(dead_code)]` becomes noise and gets switched off, and two of
    /// the generated names begin with the text of a protected one, so this also
    /// says paths are compared rather than substrings.
    #[test]
    fn an_unprotected_lint_is_never_reported(
        lint in unprotected_lint(),
        form in offending_form(),
        reason in reason(),
        depth in 0_usize..=3,
    ) {
        let source = in_macro_arms(&form.render(lint, &reason), depth);
        let found = scan(&source)?;
        prop_assert!(found.is_empty(), "source {:?} found {:?}", source, found);
    }

    /// Scenario: the sanctioned form, an item-scoped `expect` with a reason.
    ///
    /// Invariant: it is never an offence, for any lint. Rejecting it would push
    /// contributors towards `allow`, which never warns at any scope.
    #[test]
    fn an_item_scoped_expect_is_never_an_offence(lint in any_lint(), reason in reason()) {
        let sanctioned = format!("#[expect({lint}, reason = \"{reason}\")]\nfn probe() {{}}\n");
        let found = scan(&sanctioned)?;
        prop_assert!(found.is_empty(), "lint {:?} found {:?}", lint, found);
    }

    /// Scenario: attribute-shaped text in a string literal, a doc comment and a
    /// macro argument.
    ///
    /// Invariant: none is ever an offence. A string literal is a single token,
    /// so the property parsing gave us survives the drop to tokens; this says
    /// so for every lint name and every nesting depth, not just the sampled
    /// ones.
    #[test]
    fn attribute_shaped_text_is_never_an_offence(lint in any_lint(), depth in 0_usize..=3) {
        let prose = format!(
            "//! Never write #![allow({lint})] in a crate root.\n\
             const EXAMPLE: &str = \"#[allow({lint})]\";\n\
             fn describe() {{ println!(\"nor #![allow({lint})] here\"); }}\n"
        );
        let found = scan(&in_macro_arms(&prose, depth))?;
        prop_assert!(found.is_empty(), "lint {:?} found {:?}", lint, found);
    }
}
