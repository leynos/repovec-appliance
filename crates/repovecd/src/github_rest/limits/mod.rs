//! Resource budgets for one GitHub REST listing traversal.
//!
//! Every traversal is finite by construction. The pages it will request, the
//! records it will accumulate, the body of a single response, the cumulative
//! accepted body size, the `Link` header value it will parse, and the
//! continuation identities it retains are each bounded. Reaching a bound is a
//! typed failure, never a truncated listing reported as success.
//!
//! Bytes are the bound because they are what the client actually reads. A body
//! budget is not a resident-memory guarantee: allocator overhead, the decoded
//! records, and the retained continuation identities sit alongside it.

use repovec_core::repository_listing::RepositoryListingError;

/// Pages one traversal may request before the page budget is exhausted.
pub(crate) const DEFAULT_MAX_PAGES: u64 = 10_000;

/// Records one traversal may accumulate across accepted pages.
pub(crate) const DEFAULT_MAX_RECORDS: u64 = 1_000_000;

/// Bytes accepted from one response body.
pub(crate) const DEFAULT_MAX_PAGE_BYTES: u64 = 8 * 1024 * 1024;

/// Bytes accepted across every response body in one traversal.
pub(crate) const DEFAULT_MAX_TOTAL_BYTES: u64 = 64 * 1024 * 1024;

/// Bytes accepted in one `Link` header value before it is parsed.
pub(crate) const DEFAULT_MAX_LINK_HEADER_BYTES: usize = 16 * 1024;

/// Bytes retained for one canonical continuation identity.
pub(crate) const DEFAULT_MAX_CONTINUATION_IDENTITY_BYTES: usize = 16 * 1024;

/// Budgets bounding one listing traversal.
///
/// The defaults bound a full GitHub traversal comfortably while keeping a
/// hostile or malfunctioning endpoint finite. Tests construct smaller budgets
/// rather than waiting out production-sized limits.
///
/// # Examples
///
/// ```
/// use repovecd::github_repository_client::ListingLimits;
///
/// let limits = ListingLimits::defaults().with_max_pages(2);
///
/// assert_eq!(limits.max_pages(), 2);
/// assert_eq!(ListingLimits::defaults().max_pages(), 10_000);
/// ```
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ListingLimits {
    /// Pages one traversal may request.
    pub(crate) pages: u64,
    /// Records one traversal may accumulate.
    pub(crate) records: u64,
    /// Bytes accepted from one response body.
    pub(crate) page_bytes: u64,
    /// Bytes accepted across every response body.
    pub(crate) total_bytes: u64,
    /// Bytes accepted in one `Link` header value.
    pub(crate) link_header_bytes: usize,
    /// Bytes retained for one continuation identity.
    pub(crate) continuation_identity_bytes: usize,
}

impl ListingLimits {
    /// Returns the documented production budgets.
    #[must_use]
    pub const fn defaults() -> Self {
        Self {
            pages: DEFAULT_MAX_PAGES,
            records: DEFAULT_MAX_RECORDS,
            page_bytes: DEFAULT_MAX_PAGE_BYTES,
            total_bytes: DEFAULT_MAX_TOTAL_BYTES,
            link_header_bytes: DEFAULT_MAX_LINK_HEADER_BYTES,
            continuation_identity_bytes: DEFAULT_MAX_CONTINUATION_IDENTITY_BYTES,
        }
    }

    /// Returns these budgets with a different page budget.
    #[must_use]
    pub const fn with_max_pages(mut self, pages: u64) -> Self {
        self.pages = pages;
        self
    }

    /// Returns these budgets with a different record budget.
    #[must_use]
    pub const fn with_max_records(mut self, records: u64) -> Self {
        self.records = records;
        self
    }

    /// Returns these budgets with a different per-body byte budget.
    #[must_use]
    pub const fn with_max_page_bytes(mut self, page_bytes: u64) -> Self {
        self.page_bytes = page_bytes;
        self
    }

    /// Returns these budgets with a different cumulative byte budget.
    #[must_use]
    pub const fn with_max_total_bytes(mut self, total_bytes: u64) -> Self {
        self.total_bytes = total_bytes;
        self
    }

    /// Returns these budgets with a different `Link` header byte budget.
    #[must_use]
    pub const fn with_max_link_header_bytes(mut self, link_header_bytes: usize) -> Self {
        self.link_header_bytes = link_header_bytes;
        self
    }

    /// Returns these budgets with a different continuation identity budget.
    #[must_use]
    pub const fn with_max_continuation_identity_bytes(
        mut self,
        continuation_identity_bytes: usize,
    ) -> Self {
        self.continuation_identity_bytes = continuation_identity_bytes;
        self
    }

    /// Returns the configured page budget.
    #[must_use]
    pub const fn max_pages(self) -> u64 { self.pages }

    /// Returns the configured record budget.
    #[must_use]
    pub const fn max_records(self) -> u64 { self.records }

    /// Returns the configured per-body byte budget.
    #[must_use]
    pub const fn max_page_bytes(self) -> u64 { self.page_bytes }

    /// Returns the configured cumulative byte budget.
    #[must_use]
    pub const fn max_total_bytes(self) -> u64 { self.total_bytes }

    /// Returns the configured `Link` header byte budget.
    #[must_use]
    pub const fn max_link_header_bytes(self) -> usize { self.link_header_bytes }

    /// Returns the configured continuation identity budget.
    #[must_use]
    pub const fn max_continuation_identity_bytes(self) -> usize { self.continuation_identity_bytes }

    /// Checks that every budget admits at least one unit of work.
    ///
    /// A zero budget would make every traversal fail without a request, which
    /// is never what a caller means, so it is rejected at construction.
    ///
    /// # Errors
    ///
    /// Returns [`RepositoryListingError::InvalidInput`] naming `listing limits`
    /// when any budget is zero.
    pub(crate) fn validate(self) -> Result<Self, RepositoryListingError> {
        let budgets = [self.pages, self.records, self.page_bytes, self.total_bytes];
        let byte_limits = [self.link_header_bytes, self.continuation_identity_bytes];
        if budgets.contains(&0) || byte_limits.contains(&0) {
            return Err(RepositoryListingError::InvalidInput {
                field: "listing limits",
                reason: "every budget must be positive",
            });
        }
        Ok(self)
    }
}

impl Default for ListingLimits {
    fn default() -> Self { Self::defaults() }
}

#[cfg(test)]
mod tests;
