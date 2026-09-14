//! Attribute and token scanning for the environment-policy source contract.
//!
//! The judgement lives here and nowhere else: one function decides what a
//! `Meta` suppresses, whether it was parsed from the syntax tree or recovered
//! from a macro's token stream, and one filter decides which of those names the
//! policy protects. The test root records the routes that made each of those
//! decisions necessary, and the mutations that prove them.

use proc_macro2::{Delimiter, TokenStream, TokenTree};
use syn::{
    AttrStyle, Attribute, Macro, Meta, MetaList, Path, Token, ext::IdentExt,
    punctuated::Punctuated, visit::Visit,
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
pub const PROTECTED_LINTS: [&str; 7] = [
    "clippy::disallowed_methods",
    "clippy::style",
    "clippy::all",
    "warnings",
    "clippy::allow_attributes",
    "clippy::allow_attributes_without_reason",
    "clippy::restriction",
];

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
/// path in `#[allow(warnings)]` as `warnings`. Raw identifiers are
/// unwrapped, so `r#allow` renders as `allow` and `clippy::r#style` as
/// `clippy::style`. Clippy honours both spellings, so they must not be
/// two different things here.
fn render_path(path: &Path) -> String {
    path.segments
        .iter()
        .map(|segment| segment.ident.unraw().to_string())
        .collect::<Vec<_>>()
        .join("::")
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
fn suppressed_by(attribute: &Attribute) -> Vec<String> {
    suppressed_by_meta(&attribute.meta, Scope::of(attribute))
}

/// Return the lint names one attribute's meta suppresses.
///
/// This is the single judgement applied to an attribute however it was
/// found: parsed from the syntax tree, or recovered from a macro's token
/// stream. Keeping one function means the two routes cannot drift.
///
/// # Examples
///
/// The meta of `allow(warnings)` yields `["warnings"]` at either scope.
/// That of `expect(clippy::all)` yields nothing at `Scope::Item`, the
/// sanctioned form, and `["clippy::all"]` at `Scope::Crate`, where the
/// warning that makes `expect` self-removing never fires.
fn suppressed_by_meta(meta: &Meta, scope: Scope) -> Vec<String> {
    let Meta::List(list) = meta else {
        return Vec::new();
    };
    match render_path(&list.path).as_str() {
        "allow" => allowed_lints(list),
        "expect" if scope == Scope::Crate => allowed_lints(list),
        "cfg_attr" => suppressed_by_cfg_attr(list, scope),
        _ => Vec::new(),
    }
}

/// Whether an attribute applies to the item it precedes, or to
/// everything inside the module or crate that carries it.
///
/// The distinction is what makes `expect` safe at one scope and not the
/// other. An item-scoped `#[expect(...)]` warns once its item no longer
/// needs it, which is why the taxonomy sanctions it. A crate-scoped
/// `#![expect(...)]` is fulfilled by any single call anywhere beneath
/// it, so it neither reports nor goes unfulfilled. Measured on this
/// branch: `#![expect(clippy::disallowed_methods, reason = "probe")]`
/// over a `std::env::var` call produced no diagnostic of either kind. At
/// that scope `expect` is an `allow` that looks responsible.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Scope {
    /// An inner attribute, applying to everything beneath it.
    Crate,
    /// An outer attribute, applying to the item that follows it.
    Item,
}

impl Scope {
    /// Return the scope an attribute applies at.
    ///
    /// # Examples
    ///
    /// `#![allow(warnings)]` is `Crate`; `#[allow(warnings)]` is `Item`.
    const fn of(attribute: &Attribute) -> Self {
        match attribute.style {
            AttrStyle::Inner(_) => Self::Crate,
            AttrStyle::Outer => Self::Item,
        }
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
        let Some(lints) = attribute_at(&trees, index) else {
            continue;
        };
        found.extend(lints);
    }
    found
}

/// Return the lints an attribute beginning at `index` suppresses, each
/// paired with the attribute as written.
///
/// The names are returned unfiltered. Deciding which of them the policy
/// protects belongs to [`suppressed_lints`], which applies that filter
/// once over the parsed and the token-recovered findings together, so a
/// rule added to one route cannot go missing from the other.
///
/// # Examples
///
/// At the `#` of `#[allow(warnings)]` this returns one pair naming
/// `warnings`; anywhere else it returns nothing.
fn attribute_at(trees: &[TokenTree], index: usize) -> Option<Vec<(String, String)>> {
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
    let scope = if bang.is_empty() { Scope::Item } else { Scope::Crate };
    let rendered = format!("#{bang}[{}]", group.stream());
    Some(
        suppressed_by_meta(&meta, scope).into_iter().map(|lint| (lint, rendered.clone())).collect(),
    )
}

/// Return the lint names nested inside a `cfg_attr`.
///
/// # Examples
///
/// For `cfg_attr(all(), allow(clippy::style))` this returns
/// `["clippy::style"]`. The leading element is the condition and is
/// skipped; a nested `cfg_attr` is followed in turn. The outermost
/// attribute's scope is carried down, so an inner `cfg_attr` wrapping an
/// `expect` is judged as the crate-scoped suppression it becomes.
fn suppressed_by_cfg_attr(list: &MetaList, scope: Scope) -> Vec<String> {
    let Ok(nested) = list.parse_args_with(Punctuated::<Meta, Token![,]>::parse_terminated) else {
        return Vec::new();
    };
    nested.iter().skip(1).flat_map(|meta| suppressed_by_meta(meta, scope)).collect()
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
/// This is where the protected set is applied, once, over the parsed
/// attributes and the token-recovered ones together.
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
pub fn suppressed_lints(contents: &str) -> Result<Vec<(String, String)>, String> {
    let parsed = syn::parse_file(contents).map_err(|error| format!("parse: {error}"))?;
    let mut collector = AttributeCollector::default();
    collector.visit_file(&parsed);

    let mut found = Vec::new();
    for attribute in &collector.attributes {
        for lint in suppressed_by(attribute) {
            found.push((lint, render_attribute(attribute)));
        }
    }
    found.extend(collector.from_macros);
    found.retain(|(lint, _)| PROTECTED_LINTS.contains(&lint.as_str()));
    Ok(found)
}
