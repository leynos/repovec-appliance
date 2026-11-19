//! Selection and validation of the `Link` header's `next` continuation.
//!
//! The grammar belongs to `nom-rfc8288`; this module keeps that crate private
//! and owns everything the traversal needs on top of it: which `next` link is
//! unambiguous, where it may point, and how a repeated request is recognized.
//! A continuation is validated before a credential is attached to it.

use std::{collections::BTreeSet, fmt::Write as _};

use nom_rfc8288::complete::{LinkData, link_strict};
use repovec_core::repository_listing::{RepositoryListingError, ResourceBudget};
use reqwest::{Url, header::HeaderValue};

/// The request context a continuation must stay within.
#[derive(Clone, Copy, Debug)]
pub(crate) struct ContinuationContext<'a> {
    /// The validated initial request URL.
    pub(crate) initial: &'a Url,
    /// Query pairs fixed for the whole traversal.
    pub(crate) fixed_query: &'static [(&'static str, &'static str)],
    /// Bytes accepted in one `Link` header value.
    pub(crate) max_header_bytes: usize,
    /// Bytes retained for one canonical continuation identity.
    pub(crate) max_identity_bytes: usize,
}

/// Returns the next continuation, or `None` when the listing is complete.
///
/// Every header value is parsed separately and the results concatenated, so
/// several `Link` headers and one comma-joined header behave alike.
///
/// # Errors
///
/// Returns [`RepositoryListingError::InvalidContinuation`] for a malformed or
/// ambiguous header, an empty list element, or a target outside the listing.
/// Returns [`RepositoryListingError::ResourceLimitExceeded`] when a header or
/// retained identity exceeds its budget.
pub(crate) fn next_continuation(
    headers: &[HeaderValue],
    current: &Url,
    context: &ContinuationContext<'_>,
) -> Result<Option<Url>, RepositoryListingError> {
    let targets = next_targets(headers, context)?;
    match targets.as_slice() {
        [] => Ok(None),
        [target] => resolve(target, current, context).map(Some),
        _ => Err(invalid("more than one next link")),
    }
}

/// Collects the `next` targets named by every header value.
fn next_targets<'a>(
    headers: &'a [HeaderValue],
    context: &ContinuationContext<'_>,
) -> Result<Vec<&'a str>, RepositoryListingError> {
    let mut targets = Vec::new();
    for header in headers {
        let text = header
            .to_str()
            .map_err(|_| invalid("Link header is not valid text"))?;
        if text.len() > context.max_header_bytes {
            return Err(limit(
                ResourceBudget::LinkHeaderBytes,
                context.max_header_bytes,
            ));
        }
        let parsed = link_strict(text).map_err(|_| invalid("Link header is malformed"))?;
        for element in parsed {
            let Some(data) = element else {
                return Err(invalid("Link header contains an empty list element"));
            };
            if let Some(target) = next_target(&data)? {
                targets.push(target);
            }
        }
    }
    Ok(targets)
}

/// Returns the target of one link-value when its first `rel` names `next`.
fn next_target<'a>(data: &LinkData<'a>) -> Result<Option<&'a str>, RepositoryListingError> {
    let relation = data.params.iter().find(|param| param.key == "rel");
    let Some(value) = relation.and_then(|param| param.val.as_deref()) else {
        return Ok(None);
    };
    if !value.split(' ').any(|token| token == "next") {
        return Ok(None);
    }
    if data.params.iter().any(|param| param.key == "anchor") {
        return Err(invalid("an anchored next link has no listing context"));
    }
    Ok(Some(data.url))
}

/// Resolves a target against the current request and confines it.
fn resolve(
    target: &str,
    current: &Url,
    context: &ContinuationContext<'_>,
) -> Result<Url, RepositoryListingError> {
    let url = current
        .join(target)
        .map_err(|_| invalid("next link is not a valid URL"))?;
    ensure_confined(&url, context)?;
    Ok(url)
}

/// Checks a continuation stays on the listing's origin, path and filters.
fn ensure_confined(
    url: &Url,
    context: &ContinuationContext<'_>,
) -> Result<(), RepositoryListingError> {
    let initial = context.initial;
    if !url.username().is_empty() || url.password().is_some() {
        return Err(invalid("next link carries user information"));
    }
    if url.fragment().is_some() {
        return Err(invalid("next link carries a fragment"));
    }
    if origin_of(url) != origin_of(initial) {
        return Err(invalid("next link leaves the listing origin"));
    }
    if url.path() != initial.path() {
        return Err(invalid("next link changes the listing path"));
    }
    ensure_query(url, context.fixed_query)
}

/// Returns the scheme, host, and effective port a URL addresses.
///
/// Comparing the three as one value keeps the confinement check a single
/// comparison, which reads as the question it asks: is this the same origin?
fn origin_of(url: &Url) -> (&str, Option<&str>, Option<u16>) {
    (url.scheme(), url.host_str(), url.port_or_known_default())
}

/// Checks the continuation keeps every fixed query pair and changes only `page`.
fn ensure_query(
    url: &Url,
    fixed_query: &[(&'static str, &'static str)],
) -> Result<(), RepositoryListingError> {
    let mut seen = BTreeSet::new();
    for (key, value) in url.query_pairs() {
        if !seen.insert(key.to_string()) {
            return Err(invalid("next link repeats a query parameter"));
        }
        if key == "page" {
            continue;
        }
        let expected =
            fixed_value(fixed_query, &key).ok_or(invalid("next link adds a query parameter"))?;
        if expected != value.as_ref() {
            return Err(invalid("next link changes a fixed query parameter"));
        }
    }
    if fixed_query.iter().any(|(key, _)| !seen.contains(*key)) {
        return Err(invalid("next link drops a fixed query parameter"));
    }
    Ok(())
}

/// Returns the value a fixed query pair supplies for `key`.
fn fixed_value(fixed_query: &[(&'static str, &'static str)], key: &str) -> Option<&'static str> {
    fixed_query
        .iter()
        .find(|(name, _)| *name == key)
        .map(|(_, value)| *value)
}

/// Builds the canonical identity a visited-request set is keyed by.
///
/// Query pairs are sorted and length-prefixed so the identity depends on the
/// decoded values alone, not on their order or encoding in the link text.
///
/// # Errors
///
/// Returns [`RepositoryListingError::ResourceLimitExceeded`] when the identity
/// exceeds its budget.
pub(crate) fn continuation_identity(
    url: &Url,
    context: &ContinuationContext<'_>,
) -> Result<String, RepositoryListingError> {
    let mut pairs = url
        .query_pairs()
        .map(|(key, value)| (key.into_owned(), value.into_owned()))
        .collect::<Vec<_>>();
    pairs.sort();

    let parameters = pairs.iter().fold(String::new(), |mut parameters, pair| {
        // Writing into a `String` cannot fail, so the result carries nothing.
        write!(
            parameters,
            "?{}:{}={}:{}",
            pair.0.len(),
            pair.0,
            pair.1.len(),
            pair.1
        )
        .unwrap_or(());
        parameters
    });
    let identity = format!(
        "{}://{}:{}{}{}",
        url.scheme(),
        url.host_str().unwrap_or_default(),
        url.port_or_known_default().unwrap_or_default(),
        url.path(),
        parameters
    );
    if identity.len() > context.max_identity_bytes {
        return Err(limit(
            ResourceBudget::ContinuationIdentityBytes,
            context.max_identity_bytes,
        ));
    }
    Ok(identity)
}

/// Builds an invalid-continuation error with a fixed reason.
const fn invalid(reason: &'static str) -> RepositoryListingError {
    RepositoryListingError::InvalidContinuation { reason }
}

/// Builds a resource-limit error for the exhausted byte budget.
fn limit(budget: ResourceBudget, limit: usize) -> RepositoryListingError {
    RepositoryListingError::ResourceLimitExceeded {
        budget,
        limit: u64::try_from(limit).unwrap_or(u64::MAX),
    }
}

#[cfg(test)]
mod tests;
