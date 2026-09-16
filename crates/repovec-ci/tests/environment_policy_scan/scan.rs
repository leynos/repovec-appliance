//! Attribute and token scanning for the environment-policy source contract.
//!
//! The judgement lives here and nowhere else: one function decides what a
//! `Meta` suppresses, whether it was parsed from the syntax tree or recovered
//! from a macro's token stream, and one filter decides which of those names the
//! policy protects. The test root records the routes that made each of those
//! decisions necessary, and the mutations that prove them.

use syn::{
    AttrStyle, Attribute, Macro, Meta, MetaList, Path, Token, ext::IdentExt,
    punctuated::Punctuated, visit::Visit,
};

use crate::tokens::{includes_a_scanned_path, suppressions_in_tokens, transcriber_arms};

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

/// Stands in for the lint name when an attribute's body is decided elsewhere.
///
/// An attribute written `#[$attr]` in a `macro_rules!` arm names no lint: the
/// call site supplies one. Measured with Clippy on a probe crate,
/// `forward!(allow(clippy::disallowed_methods))` over a `std::env::var` call
/// silences it, the unforwarded call beside it is still reported, and
/// `clippy::allow_attributes` says nothing about either. Neither half of that
/// construction is visible on its own, so it is refused rather than resolved.
pub const FORWARDED_ATTRIBUTE: &str = "whatever the call site passes";

/// Stands in for the lint name when a source pulls in code the scan cannot see.
///
/// `include!` resolves a path, not a module, and rustc parses the target as
/// Rust whatever its extension. Measured with Clippy on a probe crate:
/// `include!("policy.rs.txt")` compiled an `#[allow(clippy::disallowed_methods,
/// reason = "..")]` inside the target, which silenced a `std::env::var` call
/// there, while an enclosing `#[expect(clippy::allow_attributes, reason = "..")]`
/// kept the guard quiet. The scan reads `.rs` files, so it never saw the target.
pub const INCLUDED_SOURCE: &str = "code from a file the scan does not read";

/// Whether a finding is one this contract reports.
fn is_reportable(lint: &str) -> bool {
    PROTECTED_LINTS.contains(&lint) || lint == FORWARDED_ATTRIBUTE || lint == INCLUDED_SOURCE
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
        match render_path(&mac.path).as_str() {
            "macro_rules" => {
                for transcriber in transcriber_arms(&mac.tokens) {
                    self.from_macros.extend(suppressions_in_tokens(&transcriber));
                }
            }
            "include" if !includes_a_scanned_path(&mac.tokens) => {
                let rendered = format!("include!({})", mac.tokens);
                self.from_macros.push((INCLUDED_SOURCE.to_owned(), rendered));
            }
            _ => {}
        }
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
pub fn suppressed_by_meta(meta: &Meta, scope: Scope) -> Vec<String> {
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
pub enum Scope {
    /// An inner attribute, applying to everything beneath it.
    Crate,
    /// An outer attribute, applying to the item that follows it.
    Item,
}

impl Scope {
    /// Render the `!` that distinguishes an inner attribute, for a message.
    ///
    /// # Examples
    ///
    /// `Scope::Crate` renders `"!"`, so an attribute reads `#![allow(..)]`;
    /// `Scope::Item` renders the empty string.
    pub const fn bang(self) -> &'static str {
        match self {
            Self::Crate => "!",
            Self::Item => "",
        }
    }

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
    found.retain(|(lint, _)| is_reportable(lint));
    Ok(found)
}
