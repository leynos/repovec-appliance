//! Repository listing domain contract.
//!
//! A [`RepositoryListing`] principal is either an authenticated user or one
//! authenticated GitHub App installation. The port returns every repository
//! observation from one completed traversal, in request order and preserving
//! multiplicity, or a typed [`RepositoryListingError`] and no partial list.
//!
//! That all-or-nothing shape is what lets a future reconciler distinguish a
//! completed scan from a failed one: a failure never masquerades as evidence
//! that repositories have disappeared.
//!
//! The port stays free of HTTP, filesystem, environment, and GitHub JSON types.
//! Adapters in `repovecd` bind a credential, an endpoint, and resource limits at
//! construction time.
//!
//! # Examples
//!
//! ```
//! use repovec_core::repository_listing::{
//!     RepositoryId,
//!     RepositoryListing,
//!     RepositoryName,
//!     RepositoryOwner,
//!     RepositorySummary,
//! };
//!
//! struct FixedListing(Vec<RepositorySummary>);
//!
//! impl RepositoryListing for FixedListing {
//!     fn list_repositories(
//!         &self,
//!     ) -> Result<Vec<RepositorySummary>, repovec_core::repository_listing::RepositoryListingError>
//!     {
//!         Ok(self.0.clone())
//!     }
//! }
//!
//! let listing = FixedListing(vec![
//!     RepositorySummary::new(
//!         RepositoryId::new(1)?,
//!         RepositoryOwner::new("octocat")?,
//!         RepositoryName::new("hello-world")?,
//!     )
//!     .with_details(false, Some("main".to_owned())),
//! ]);
//!
//! let repositories = listing.list_repositories()?;
//!
//! assert_eq!(repositories.len(), 1);
//! assert_eq!(repositories[0].to_string(), "octocat/hello-world");
//!
//! # Ok::<(), repovec_core::repository_listing::RepositoryListingError>(())
//! ```

mod error;
mod identity;
mod summary;

#[cfg(test)]
mod tests;

pub use error::{RateLimitInfo, RepositoryListingError, ResourceBudget};
pub use identity::{RepositoryId, RepositoryName, RepositoryOwner};
pub use summary::RepositorySummary;

/// A source of repositories visible to one bound principal.
///
/// Implementations bind a credential, an endpoint, and resource limits when
/// they are constructed. A successful call represents every observation from
/// one completed traversal, not an atomic snapshot of GitHub: repositories
/// changed during the scan may be omitted or repeated.
///
/// # Examples
///
/// ```
/// use repovec_core::repository_listing::{RepositoryListing, RepositoryListingError};
///
/// struct EmptyListing;
///
/// impl RepositoryListing for EmptyListing {
///     fn list_repositories(&self) -> Result<Vec<repovec_core::repository_listing::RepositorySummary>, RepositoryListingError> {
///         Ok(Vec::new())
///     }
/// }
///
/// assert!(EmptyListing.list_repositories()?.is_empty());
///
/// # Ok::<(), RepositoryListingError>(())
/// ```
pub trait RepositoryListing {
    /// Returns every repository observation from one completed traversal.
    ///
    /// # Errors
    ///
    /// Returns a typed error when the traversal cannot complete. On error no
    /// observation is returned, so a caller never mistakes a partial traversal
    /// for a complete listing.
    fn list_repositories(&self) -> Result<Vec<RepositorySummary>, RepositoryListingError>;
}
