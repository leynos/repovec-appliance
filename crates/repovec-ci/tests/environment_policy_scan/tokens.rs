//! Attribute recovery from macro token streams, and the two constructions
//! that put an attribute beyond a parser's reach.
//!
//! `syn` keeps a `macro_rules!` body as opaque tokens, so an `allow` emitted
//! from an arm never reaches `visit_attribute` while Clippy expands and honours
//! it. Walking those tokens is what closes that route, and the rules here are
//! what keep the walk from reporting constructions that only resemble it.
//!
//! The judgement itself stays in [`crate::scan`]: a `Meta` recovered from
//! tokens is handed to the same function as one parsed from the syntax tree, so
//! the two routes cannot drift apart.
//!
//! Mutation record for the rules in this module, proved in both directions on
//! 2026-09-14 and re-run since, each applied alone and run through the build:
//!
//! - naming an unprotected lint in the forwarded finding fails
//!   `a_suppression_is_an_offence::case_10_forwarded_from_a_macro_argument`;
//! - widening `is_forwarded` to every unparsable attribute body fails
//!   `a_construction_that_only_resembles_a_route_is_not_an_offence::
//!   case_2_forwarded_doc_and_derive`;
//! - walking every macro's whole token stream, as this contract did before,
//!   fails the consumed-argument and matcher-only cases;
//! - walking a `macro_rules!` matcher as well as its transcriber fails the
//!   matcher-only case.
//!
//! The `include!` rule keeps its own record on
//! [`includes_a_scanned_path`], which is where its three mutations live.

use camino::{Utf8Component, Utf8Path};
use proc_macro2::{Delimiter, TokenStream, TokenTree};
use syn::{LitStr, Meta, ext::IdentExt};

use crate::{
    scan::{FORWARDED_ATTRIBUTE, Scope, suppressed_by_meta},
    sources::SOURCE_EXTENSION,
};

/// Whether an unparsable attribute body is one the call site decides.
///
/// Two shapes qualify, and only those two. A body beginning with `$` is an
/// attribute chosen entirely at the call site, as in `#[$attr]`. A body
/// beginning with `allow`, `expect` or `cfg_attr` is known to be
/// suppression-shaped while its arguments cannot be read, as in
/// `#[allow($lint)]` or `#[cfg_attr($cond, allow(clippy::style))]`.
///
/// Everything else is left alone, which is what keeps this from reporting the
/// forwarding idioms that have nothing to do with the policy: `#[doc = $doc]`
/// and `#[derive($trait)]` both fail to parse as a `Meta`, and a rule that
/// reported them would be switched off within a week.
fn is_forwarded(body: &TokenStream) -> bool {
    match body.clone().into_iter().next() {
        Some(TokenTree::Punct(punct)) => punct.as_char() == '$',
        Some(TokenTree::Ident(ident)) => {
            matches!(ident.unraw().to_string().as_str(), "allow" | "expect" | "cfg_attr")
        }
        Some(TokenTree::Group(_) | TokenTree::Literal(_)) | None => false,
    }
}

/// Whether an `include!` target is a path the scan already reads.
///
/// Only a literal `.rs` path qualifies. A computed target, such as the
/// `concat!(env!("OUT_DIR"), "/generated.rs")` build-script idiom, cannot be
/// resolved here and is reported, so that generated code has to be brought
/// under the policy deliberately rather than by an extension nobody checks.
///
/// The target is parsed as a string literal and its decoded value tested, not
/// its source spelling. `Literal::to_string` renders a literal as it was
/// written, so `r"support.rs"` and `"support\x2Ers"` both name a file the scan
/// reads while neither is spelled `"..\.rs"`; judging the spelling reported
/// them, and a contract that reports a false positive gets switched off.
/// Parsing keeps the strictness that matters: a stream that is not exactly one
/// string literal does not parse, so a computed target is still refused.
///
/// The decoded value is tested with the same extension comparison
/// [`crate::sources::rust_sources`] selects files by, and against the same
/// constant, so "a path the scan reads" cannot come to mean two things.
///
/// The extension alone is not enough, because `include!` resolves its target
/// relative to the including file and the scan walks only the source roots.
/// `include!("../../outside/policy.rs")` names a `.rs` file the scan never
/// reads, so a suppression there would be invisible. The target must therefore
/// stay inside the tree it is included from: every component normal or `.`,
/// which rules out a parent component and any root or prefix. A relative path
/// without `..` cannot leave the root its including file sits under, and
/// `rust_sources` reads every `.rs` beneath that root, so membership of the
/// discovered set follows rather than needing to be checked. A backslash is
/// refused outright, since it separates components on the platform where
/// rustc would resolve it and is an ordinary filename character here.
///
/// Mutation proof, recorded 2026-09-15 and re-run against the shared extension
/// comparison; each applied alone here and run through the build:
///
/// - reading the rendered literal for a `"` opener and a `.rs"` close, as this
///   contract did before, fails the raw-literal and escaped-literal cases of
///   `a_construction_that_only_resembles_a_route_is_not_an_offence`;
/// - accepting a target containing a scanned-looking literal anywhere, rather
///   than one that is exactly such a literal, fails
///   `a_suppression_is_an_offence::case_15_included_from_a_concatenated_path`
///   alone, which earns that case its line;
/// - returning `true` for every target fails every `include!` case of that
///   test;
/// - dropping the containment test, so the extension alone decides, fails
///   `case_13_included_outside_the_scanned_tree` and
///   `case_14_included_from_an_absolute_path`, which is the escape route
///   review found;
/// - refusing any target with a separator in it, a containment test drawn one
///   step too wide, fails
///   `a_construction_that_only_resembles_a_route_is_not_an_offence::
///   case_6_included_scanned_subdirectory`.
///
/// Two widenings are recorded without a fixture, because no fixture
/// distinguishes them and one that survives its own mutation proves nothing.
/// Reaching only the last top-level token left every case green, since
/// `concat!` nests its literals in a group.
///
/// The concatenated case had to be added when containment landed. Until then
/// the computed-path case carried the "exactly one literal" rule, but its
/// inner literal is `/generated.rs`, which containment now refuses on its own,
/// so the widening stopped failing anything. Every literal in the concatenated
/// case names a path the scan would read, which is what restores the
/// distinction. A rule's mutations are only as good as the last mechanism
/// change they were re-run against.
pub fn includes_a_scanned_path(tokens: &TokenStream) -> bool {
    syn::parse2::<LitStr>(tokens.clone()).is_ok_and(|literal| is_scanned_path(&literal.value()))
}

/// Whether a decoded `include!` target names a file the scan reads.
///
/// # Examples
///
/// `support.rs` and `gen/support.rs` qualify; `../outside.rs`,
/// `/etc/policy.rs`, `..\outside.rs` and `support.rs.txt` do not.
fn is_scanned_path(value: &str) -> bool {
    let path = Utf8Path::new(value);
    path.extension() == Some(SOURCE_EXTENSION) && !value.contains('\\') && stays_inside(path)
}

/// Whether a relative path can only descend from where it starts.
///
/// # Examples
///
/// `gen/support.rs` and `./support.rs` stay inside; `../support.rs` and
/// `/support.rs` do not.
fn stays_inside(path: &Utf8Path) -> bool {
    path.components()
        .all(|component| matches!(component, Utf8Component::Normal(_) | Utf8Component::CurDir))
}

/// Return the transcriber of each arm of a `macro_rules!` body.
///
/// An arm is `(matcher) => {transcriber};`, and only the transcriber is
/// written out. Scanning the matcher too would report
/// `some_macro!(#[allow(clippy::style)])` as a suppression even where the
/// macro consumes the attribute and emits nothing, which is a false positive,
/// and a contract that reports one gets switched off.
///
/// Restricting the walk this way does not reopen the route it was added for. An
/// attribute passed in as a macro argument and then emitted must pass through a
/// `#[$meta]` in the definition, which `is_forwarded` refuses. An attribute
/// synthesized by a procedural macro remains out of reach, as it always was.
pub fn transcriber_arms(tokens: &TokenStream) -> Vec<TokenStream> {
    let trees: Vec<TokenTree> = tokens.clone().into_iter().collect();
    let mut arms = Vec::new();
    let mut index = 0_usize;

    while index < trees.len() {
        let next = index.saturating_add(1);
        let after = index.saturating_add(2);
        let body = index.saturating_add(3);
        let is_arm = matches!(trees.get(index), Some(TokenTree::Group(_)))
            && matches!(trees.get(next), Some(TokenTree::Punct(punct)) if punct.as_char() == '=')
            && matches!(trees.get(after), Some(TokenTree::Punct(punct)) if punct.as_char() == '>');
        if let (true, Some(TokenTree::Group(transcriber))) = (is_arm, trees.get(body)) {
            arms.push(transcriber.stream());
            index = body.saturating_add(1);
        } else {
            index = next;
        }
    }
    arms
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
pub fn suppressions_in_tokens(tokens: &TokenStream) -> Vec<(String, String)> {
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
    let (body, scope) = attribute_body_at(trees, index)?;
    let rendered = format!("#{}[{body}]", scope.bang());
    let Ok(meta) = syn::parse2::<Meta>(body.clone()) else {
        return is_forwarded(&body).then(|| vec![(FORWARDED_ATTRIBUTE.to_owned(), rendered)]);
    };
    Some(
        suppressed_by_meta(&meta, scope).into_iter().map(|lint| (lint, rendered.clone())).collect(),
    )
}

/// Locate the bracketed body of an attribute beginning at `index`.
///
/// Returns the body and the scope the attribute applies at, or `None` when the
/// `#` began something that is not an attribute. Reading the shape is kept
/// apart from judging it so that neither half grows past the complexity
/// ceiling the repository's code-health gate enforces.
///
/// # Examples
///
/// At the `#` of `#![allow(warnings)]` this returns the tokens
/// `allow(warnings)` and `Scope::Crate`; at the `#` of `#[allow(warnings)]`,
/// the same tokens and `Scope::Item`.
fn attribute_body_at(trees: &[TokenTree], index: usize) -> Option<(TokenStream, Scope)> {
    match trees.get(index) {
        Some(TokenTree::Punct(punct)) if punct.as_char() == '#' => {}
        _ => return None,
    }

    let mut next = index.checked_add(1)?;
    let mut scope = Scope::Item;
    if matches!(trees.get(next), Some(TokenTree::Punct(punct)) if punct.as_char() == '!') {
        scope = Scope::Crate;
        next = next.checked_add(1)?;
    }

    let Some(TokenTree::Group(group)) = trees.get(next) else {
        return None;
    };
    (group.delimiter() == Delimiter::Bracket).then(|| (group.stream(), scope))
}
