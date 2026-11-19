//! Descriptions of how one GitHub REST listing pages.
//!
//! A profile names the path, the query fixed for the whole traversal, and
//! where a page keeps its items. Everything that varies between listings lives
//! here, so the traversal itself carries no knowledge of repositories, branches
//! or pull requests.

use std::marker::PhantomData;

/// Where a page body keeps its items.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum PageItems {
    /// The body is a JSON array of items.
    BareArray,
    /// The body is a JSON object holding the items under this key.
    Envelope(&'static str),
}

/// The fixed description of one paginated GitHub REST listing.
#[derive(Debug)]
pub(crate) struct EndpointProfile<T> {
    /// Path below the API origin, for example `/user/repos`.
    pub(crate) path: &'static str,
    /// Query pairs fixed for the whole traversal, including `per_page=100`.
    pub(crate) fixed_query: &'static [(&'static str, &'static str)],
    /// Where a page keeps its items.
    pub(crate) items: PageItems,
    /// Marks the wire record type decoded from each item.
    pub(crate) record: PhantomData<fn() -> T>,
}

impl<T> EndpointProfile<T> {
    /// Creates the profile for one listing endpoint.
    pub(crate) const fn new(
        path: &'static str,
        fixed_query: &'static [(&'static str, &'static str)],
        items: PageItems,
    ) -> Self {
        Self {
            path,
            fixed_query,
            items,
            record: PhantomData,
        }
    }
}
