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
//! `expect` is untouched by this scan. It is the sanctioned form for a
//! composition root precisely because it warns once the site grows a
//! seam; `allow` is silent forever.
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
//! - `#[allow(clippy::alloc_instead_of_core)]` must and does keep
//!   passing, as does attribute-shaped text inside a macro argument.
//!
//! That last case is the one that keeps this contract honest. Its name
//! begins with `clippy::all`, so an earlier draft comparing lint names
//! by substring would have reported it. Names are compared as paths.

use std::collections::VecDeque;

use camino::{Utf8Path, Utf8PathBuf};
use cap_std::{ambient_authority, fs_utf8::Dir};
use proc_macro2::{Delimiter, TokenStream, TokenTree};
use syn::{
    AttrStyle, Attribute, Macro, Meta, MetaList, Path, Token, punctuated::Punctuated, visit::Visit,
};

/// Lints whose suppression disarms the environment-access policy.
///
/// Naming the policy lint alone is not enough, in two directions.
///
/// Upwards, Clippy places `disallowed_methods` in the `style` group, so
/// `clippy::style` and the wider `clippy::all` each switch it off, and
/// `warnings` takes down everything.
///
/// Sideways, the guard lints matter too. `clippy::allow_attributes` is
/// what makes the "`expect`, never `allow`" rule enforceable, and it
/// lives in the `restriction` group. Measured against Clippy: an outer
/// `#[allow(clippy::disallowed_methods)]` alone produces two
/// `allow_attributes` diagnostics, but under a crate-level
/// `#![allow(clippy::restriction)]` it produces none, and no
/// disallowed-method diagnostic either. The suppression is invisible and
/// so is the guard that would have reported it.
///
/// Every entry was confirmed against Clippy before being listed. Extend
/// this if the policy lint's group changes, or if a new lint becomes
/// load-bearing for the rule.
const PROTECTED_LINTS: [&str; 7] = [
    "clippy::disallowed_methods",
    "clippy::style",
    "clippy::all",
    "warnings",
    "clippy::allow_attributes",
    "clippy::allow_attributes_without_reason",
    "clippy::restriction",
];

/// Directories holding Rust sources the policy governs.
const SOURCE_ROOTS: [&str; 1] = ["crates"];

/// Return the repository root, from this crate's manifest directory.
fn repository_root() -> Utf8PathBuf {
    Utf8PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("..").join("..")
}

/// Collect every `.rs` file under one root, depth first.
///
/// Paths are reported relative to the repository root, because the
/// absolute one is built from this crate's manifest directory and reads
/// as `crates/repovec-ci/../../crates/...`, which is noise in a failure
/// a contributor has to act on.
fn rust_sources(root: &Utf8Path, relative: &str) -> Result<Vec<(Utf8PathBuf, String)>, String> {
    let directory = Dir::open_ambient_dir(root, ambient_authority())
        .map_err(|error| format!("open {root}: {error}"))?;
    let mut pending = VecDeque::from([(directory, Utf8PathBuf::from(relative))]);
    let mut sources = Vec::new();

    while let Some((current, prefix)) = pending.pop_front() {
        let entries = current.entries().map_err(|error| format!("read {prefix}: {error}"))?;
        for candidate in entries {
            let entry = candidate.map_err(|error| format!("entry under {prefix}: {error}"))?;
            let name = entry.file_name().map_err(|error| format!("name: {error}"))?;
            let path = prefix.join(&name);
            let file_type =
                entry.file_type().map_err(|error| format!("type of {path}: {error}"))?;
            if file_type.is_dir() {
                let child =
                    current.open_dir(&name).map_err(|error| format!("open {path}: {error}"))?;
                pending.push_back((child, path));
            } else if path.extension() == Some("rs") {
                let contents = current
                    .read_to_string(&name)
                    .map_err(|error| format!("read {path}: {error}"))?;
                sources.push((path, contents));
            }
        }
    }
    Ok(sources)
}

/// Collect every attribute in a parsed file, wherever it sits.
///
/// A visitor is used rather than a hand-rolled walk so that attributes
/// on nested items, on function-local items and on expressions are all
/// reached.
#[derive(Default)]
struct AttributeCollector {
    attributes: Vec<Attribute>,
    from_macros: Vec<(String, String)>,
}

impl<'ast> Visit<'ast> for AttributeCollector {
    fn visit_attribute(&mut self, attribute: &'ast Attribute) {
        self.attributes.push(attribute.clone());
    }

    fn visit_macro(&mut self, mac: &'ast Macro) {
        self.from_macros.extend(suppressions_in_tokens(&mac.tokens));
    }
}

/// Render a lint path as it is written in an attribute.
///
/// # Examples
///
/// The path in `#[allow(clippy::all)]` renders as `clippy::all`, and the
/// path in `#[allow(warnings)]` as `warnings`.
fn render_path(path: &Path) -> String {
    path.segments.iter().map(|segment| segment.ident.to_string()).collect::<Vec<_>>().join("::")
}

/// Return the lint names an `allow` meta-list suppresses.
///
/// Key-value arguments such as `reason = "..."` are not lint names and
/// are skipped.
///
/// # Examples
///
/// Given `allow(clippy::all, reason = "x")` this returns
/// `["clippy::all"]`; given `allow(warnings, dead_code)` it returns both
/// names.
fn allowed_lints(list: &MetaList) -> Vec<String> {
    let Ok(nested) = list.parse_args_with(Punctuated::<Meta, Token![,]>::parse_terminated) else {
        return Vec::new();
    };
    nested
        .iter()
        .filter_map(|meta| match meta {
            Meta::Path(path) => Some(render_path(path)),
            Meta::List(_) | Meta::NameValue(_) => None,
        })
        .collect()
}

/// Return the lint names one attribute suppresses, following `cfg_attr`.
///
/// A `cfg_attr` is followed whatever its condition. A suppression that
/// applies under some configuration is still a suppression, and deciding
/// which configurations are reachable is not this contract's job.
///
/// # Examples
///
/// `#[allow(warnings)]` yields `["warnings"]`.
/// `#[cfg_attr(unix, allow(clippy::all))]` also yields `["clippy::all"]`,
/// because the nested attribute is followed.
/// `#[expect(clippy::disallowed_methods, reason = "...")]` yields nothing,
/// because `expect` is the sanctioned form.
fn suppressed_by(attribute: &Attribute) -> Vec<String> { suppressed_by_meta(&attribute.meta) }

/// Return the lint names one attribute's meta suppresses.
///
/// This is the single judgement applied to an attribute however it was
/// found: parsed from the syntax tree, or recovered from a macro's token
/// stream. Keeping one function means the two routes cannot drift.
///
/// # Examples
///
/// The meta of `allow(warnings)` yields `["warnings"]`; that of
/// `expect(clippy::all)` yields nothing.
fn suppressed_by_meta(meta: &Meta) -> Vec<String> {
    let Meta::List(list) = meta else {
        return Vec::new();
    };
    match render_path(&list.path).as_str() {
        "allow" => allowed_lints(list),
        "cfg_attr" => suppressed_by_cfg_attr(list),
        _ => Vec::new(),
    }
}

/// Return the protected lints suppressed by attributes inside a macro's
/// token stream.
///
/// `macro_rules!` arms are opaque to the syntax tree: `syn` keeps the
/// body as tokens, so an `allow` emitted from an arm never reaches
/// `visit_attribute`, while Clippy expands and honours it. Measured on
/// this branch: an arm emitting
/// `#[allow(clippy::disallowed_methods)]` over a `std::env::var` call
/// produced no disallowed-method diagnostic at all.
///
/// The walk looks for `#` optionally followed by `!` and then a bracket
/// group, parses that group as a `Meta`, and applies the same judgement
/// as a parsed attribute. Every group is recursed into, so an arm that
/// defines another macro is covered too.
///
/// A string literal is a single token, so text that looks like an
/// attribute inside one is never mistaken for a suppression. That is the
/// same property parsing gave us, kept rather than given back.
///
/// # Examples
///
/// Tokens for `{ () => { #[allow(warnings)] fn f() {} }; }` yield
/// `warnings`; tokens for `{ "#[allow(warnings)]" }` yield nothing.
fn suppressions_in_tokens(tokens: &TokenStream) -> Vec<(String, String)> {
    let trees: Vec<TokenTree> = tokens.clone().into_iter().collect();
    let mut found = Vec::new();

    for (index, tree) in trees.iter().enumerate() {
        if let TokenTree::Group(group) = tree {
            found.extend(suppressions_in_tokens(&group.stream()));
        }
        let Some(attribute) = attribute_at(&trees, index) else {
            continue;
        };
        found.extend(attribute.1);
    }
    found
}

/// Return the rendered attribute and its suppressed lints at `index`.
///
/// # Examples
///
/// At the `#` of `#[allow(warnings)]` this returns the rendered
/// attribute and `["warnings"]`; anywhere else it returns nothing.
fn attribute_at(trees: &[TokenTree], index: usize) -> Option<(String, Vec<(String, String)>)> {
    match trees.get(index) {
        Some(TokenTree::Punct(punct)) if punct.as_char() == '#' => {}
        _ => return None,
    }

    let mut next = index.checked_add(1)?;
    let mut bang = "";
    if matches!(trees.get(next), Some(TokenTree::Punct(punct)) if punct.as_char() == '!') {
        bang = "!";
        next = next.checked_add(1)?;
    }

    let Some(TokenTree::Group(group)) = trees.get(next) else {
        return None;
    };
    if group.delimiter() != Delimiter::Bracket {
        return None;
    }

    let meta = syn::parse2::<Meta>(group.stream()).ok()?;
    let rendered = format!("#{bang}[{}]", group.stream());
    let lints = suppressed_by_meta(&meta)
        .into_iter()
        .filter(|lint| PROTECTED_LINTS.contains(&lint.as_str()))
        .map(|lint| (lint, rendered.clone()))
        .collect();
    Some((rendered, lints))
}

/// Return the lint names nested inside a `cfg_attr`.
///
/// # Examples
///
/// For `cfg_attr(all(), allow(clippy::style))` this returns
/// `["clippy::style"]`. The leading element is the condition and is
/// skipped; a nested `cfg_attr` is followed in turn.
fn suppressed_by_cfg_attr(list: &MetaList) -> Vec<String> {
    let Ok(nested) = list.parse_args_with(Punctuated::<Meta, Token![,]>::parse_terminated) else {
        return Vec::new();
    };
    nested.iter().skip(1).flat_map(suppressed_by_meta).collect()
}

/// Render an attribute roughly as written, for a failure message.
///
/// # Examples
///
/// An inner `allow` of `clippy::all` renders as `#![allow(clippy::all)]`,
/// and the outer form without the `!`.
fn render_attribute(attribute: &Attribute) -> String {
    let bang = match attribute.style {
        AttrStyle::Inner(_) => "!",
        AttrStyle::Outer => "",
    };
    let path = render_path(attribute.path());
    attribute.meta.require_list().map_or_else(
        |_| format!("#{bang}[{path}]"),
        |list| format!("#{bang}[{path}({})]", list.tokens),
    )
}

/// Return every protected lint suppressed in one source file.
///
/// The source is parsed rather than searched. A text scan cannot tell an
/// attribute from attribute-shaped text in a string literal, cannot
/// follow `cfg_attr`, and breaks on a parenthesis inside a `reason`.
///
/// # Examples
///
/// A file containing `#![allow(clippy::style)]` yields one offence
/// naming `clippy::style`. A file whose only mention is inside a string
/// literal or a doc comment yields none.
fn suppressed_lints(contents: &str) -> Result<Vec<(String, String)>, String> {
    let parsed = syn::parse_file(contents).map_err(|error| format!("parse: {error}"))?;
    let mut collector = AttributeCollector::default();
    collector.visit_file(&parsed);

    let mut found = Vec::new();
    for attribute in &collector.attributes {
        for lint in suppressed_by(attribute) {
            if PROTECTED_LINTS.contains(&lint.as_str()) {
                found.push((lint, render_attribute(attribute)));
            }
        }
    }
    found.extend(collector.from_macros);
    Ok(found)
}

/// Scenario: a source file switches the policy lint off for itself.
///
/// Invariant: no Rust source allows a protected lint. An inner attribute
/// is the case that matters, because `clippy::allow_attributes` cannot
/// see one, so nothing else in the repository would notice.
#[test]
fn no_source_file_allows_a_policy_lint() {
    let root = repository_root();
    let mut offences = Vec::new();

    for source_root in SOURCE_ROOTS {
        let sources = rust_sources(&root.join(source_root), source_root)
            .expect("workspace sources should be readable");
        assert!(!sources.is_empty(), "{source_root} should contain Rust sources to scan");
        for (path, contents) in sources {
            let offending = suppressed_lints(&contents)
                .unwrap_or_else(|error| panic!("{path} should parse as Rust: {error}"));
            for (lint, attribute) in offending {
                offences.push(format!("{path} allows {lint} via {attribute}"));
            }
        }
    }

    assert!(
        offences.is_empty(),
        "no source may allow a protected lint; use an item-scoped \
         #[expect(..., reason = \"...\")] at a composition root instead:\n{}",
        offences.join("\n")
    );
}

/// Scenario: the scan is pointed at sources that no longer exist.
///
/// Invariant: the scan reads real files and would fail loudly rather
/// than pass by finding nothing, which is how a source scan usually
/// rots.
#[test]
fn the_scan_reads_the_workspace_sources() {
    let root = repository_root();
    let sources = rust_sources(&root.join("crates"), "crates").expect("crates should be readable");

    assert!(
        sources.len() > 20,
        "the scan should cover the workspace's sources, found {}",
        sources.len()
    );
    assert!(
        sources.iter().any(|(path, _)| path.as_str().ends_with("repovec-ci/src/lib.rs")),
        "the scan should reach each crate's root module"
    );
}

/// Scenario: an `expect` at a sanctioned composition root.
///
/// Invariant: the scan does not reject it. Rejecting `expect` would push
/// contributors towards `allow`, which is the attribute that never
/// warns, so the scan must leave the sanctioned form alone.
#[test]
fn a_sanctioned_expect_is_not_an_offence() {
    let sanctioned = "#[expect(clippy::disallowed_methods, reason = \"composition root\")]\n\
         fn read() -> Option<String> { std::env::var(\"HOME\").ok() }\n";

    assert!(suppressed_lints(sanctioned).expect("fixture should parse").is_empty());
}

/// Scenario: the suppression is nested inside an active `cfg_attr`.
///
/// Invariant: it is reported. Clippy honours the nested `allow`, and
/// `clippy::allow_attributes` does not report it either, so a scan that
/// only looked for a line beginning `#![allow(` would miss it entirely.
/// The condition is not evaluated: a suppression that applies under some
/// configuration is still a suppression.
#[test]
fn a_suppression_nested_in_cfg_attr_is_an_offence() {
    let nested = "#![cfg_attr(all(), allow(clippy::disallowed_methods, reason = \"x\"))]\n";

    let found = suppressed_lints(nested).expect("fixture should parse");
    assert_eq!(found.len(), 1, "found {found:?}");
}

/// Scenario: the suppression names the lint's group rather than the lint.
///
/// Invariant: it is reported. Clippy places `disallowed_methods` in the
/// `style` group, so `#![allow(clippy::style)]` switches the policy off
/// while never naming it.
#[test]
fn a_suppression_of_the_lints_group_is_an_offence() {
    let group = "#![allow(clippy::style)]\n";

    let found = suppressed_lints(group).expect("fixture should parse");
    assert_eq!(found.len(), 1, "found {found:?}");
}

/// Scenario: spacing and a parenthesis inside the reason string.
///
/// Invariant: both are handled. A scan matching a fixed opener would
/// miss the spaced form, and one counting raw parentheses would end the
/// attribute early at the parenthesis inside the string.
#[test]
fn spacing_and_a_parenthesis_in_the_reason_do_not_hide_a_suppression() {
    let awkward = "#![allow (warnings, reason = \"see the note (below)\")]\n";

    let found = suppressed_lints(awkward).expect("fixture should parse");
    assert_eq!(found.len(), 1, "found {found:?}");
}

/// Scenario: attribute-shaped text inside a string literal.
///
/// Invariant: it is not an offence. This is the false positive a text
/// scan cannot avoid, and a contract that reports one gets disabled.
#[test]
fn attribute_shaped_text_in_a_string_is_not_an_offence() {
    let literal = "const EXAMPLE: &str = \"\n#![allow(warnings)]\n\";\n";

    assert!(suppressed_lints(literal).expect("fixture should parse").is_empty());
}

/// Scenario: the suppression is emitted from a `macro_rules!` arm.
///
/// Invariant: it is reported. Clippy expands the arm and honours the
/// attribute, but `syn` keeps the arm's body an opaque token stream, so
/// the attribute never reaches `visit_attribute`. Measured on this
/// branch: an arm emitting this over a `std::env::var` call produced no
/// disallowed-method diagnostic.
#[test]
fn a_suppression_emitted_from_a_macro_arm_is_an_offence() {
    let source = "macro_rules! probe {\n\
         () => {\n\
         #[allow(clippy::disallowed_methods, reason = \"x\")]\n\
         pub fn probe() -> Option<String> { std::env::var(\"HOME\").ok() }\n\
         };\n\
         }\n";

    let found = suppressed_lints(source).expect("fixture should parse");
    assert_eq!(found.len(), 1, "found {found:?}");
}

/// Scenario: the suppression is two macro definitions deep.
///
/// Invariant: it is reported. Every token group is recursed into, so an
/// arm that defines another macro does not hide the attribute its inner
/// arm emits.
#[test]
fn a_suppression_two_macro_arms_deep_is_an_offence() {
    let source = "macro_rules! outer {\n\
         () => {\n\
         macro_rules! inner {\n\
         () => {\n\
         #![allow(clippy::style)]\n\
         };\n\
         }\n\
         };\n\
         }\n";

    let found = suppressed_lints(source).expect("fixture should parse");
    assert_eq!(found.len(), 1, "found {found:?}");
}

/// Scenario: attribute-shaped text inside a macro's arguments.
///
/// Invariant: it is not an offence. A string literal is a single token,
/// so the walk cannot mistake its contents for an attribute. This is the
/// property parsing gave us, kept rather than handed back when the walk
/// dropped to tokens.
#[test]
fn attribute_shaped_text_inside_a_macro_argument_is_not_an_offence() {
    let source = "fn describe() { println!(\"never write #![allow(warnings)] here\"); }\n";

    assert!(suppressed_lints(source).expect("fixture should parse").is_empty());
}

/// Scenario: a lint whose name merely begins with a protected one.
///
/// Invariant: `clippy::alloc_instead_of_core` is not reported as
/// suppressing `clippy::all`, whose full name is a prefix of it. Lint
/// names are compared as paths, not as text. A substring test would
/// reject this, and a contributor would have no way to tell a real
/// finding from a false one.
#[test]
fn a_longer_lint_name_beginning_with_a_protected_one_is_not_an_offence() {
    let innocent = "#[allow(clippy::alloc_instead_of_core)]\n\
         fn documented() {}\n";

    assert!(suppressed_lints(innocent).expect("fixture should parse").is_empty());
}

/// Scenario: a doc comment mentions the attribute.
///
/// Invariant: prose describing the policy is not reported as breaking
/// it. This file and its sibling contracts all quote the attribute in
/// their own documentation.
#[test]
fn a_mention_in_prose_is_not_an_attribute() {
    let prose = "//! Never write #![allow(clippy::disallowed_methods)] in a crate root.\n\
         const EXAMPLE: &str = \"#[allow(warnings)]\";\n";

    assert!(suppressed_lints(prose).expect("fixture should parse").is_empty());
}
