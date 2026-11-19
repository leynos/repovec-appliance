//! One validated repository observation.

use std::fmt;

use super::{RepositoryId, RepositoryName, RepositoryOwner};

/// One repository observation returned by a completed listing traversal.
///
/// # Examples
///
/// ```
/// use repovec_core::repository_listing::{
///     RepositoryId,
///     RepositoryName,
///     RepositoryOwner,
///     RepositorySummary,
/// };
///
/// let repository = RepositorySummary::new(
///     RepositoryId::new(1)?,
///     RepositoryOwner::new("octocat")?,
///     RepositoryName::new("hello-world")?,
/// )
/// .with_details(false, Some("main".to_owned()));
///
/// assert_eq!(repository.to_string(), "octocat/hello-world");
///
/// # Ok::<(), repovec_core::repository_listing::RepositoryListingError>(())
/// ```
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RepositorySummary {
    /// Stable numeric repository identifier.
    id: RepositoryId,
    /// Account login that owns the repository.
    owner: RepositoryOwner,
    /// Repository name within the owner account.
    name: RepositoryName,
    /// Whether GitHub reports the repository as private.
    is_private: bool,
    /// Default branch name, absent for a repository with no commits.
    default_branch: Option<String>,
}

impl RepositorySummary {
    /// Creates one repository observation from its identity components.
    ///
    /// Privacy defaults to public and the default branch to absent; call
    /// [`Self::with_details`] to supply the values GitHub reported.
    ///
    /// # Examples
    ///
    /// ```
    /// use repovec_core::repository_listing::{
    ///     RepositoryId,
    ///     RepositoryName,
    ///     RepositoryOwner,
    ///     RepositorySummary,
    /// };
    ///
    /// let repository = RepositorySummary::new(
    ///     RepositoryId::new(1)?,
    ///     RepositoryOwner::new("octocat")?,
    ///     RepositoryName::new("hello-world")?,
    /// );
    ///
    /// assert!(!repository.is_private());
    /// assert_eq!(repository.default_branch(), None);
    ///
    /// # Ok::<(), repovec_core::repository_listing::RepositoryListingError>(())
    /// ```
    #[must_use]
    pub const fn new(id: RepositoryId, owner: RepositoryOwner, name: RepositoryName) -> Self {
        Self {
            id,
            owner,
            name,
            is_private: false,
            default_branch: None,
        }
    }

    /// Sets the privacy flag and default branch reported by GitHub.
    ///
    /// # Examples
    ///
    /// ```
    /// # use repovec_core::repository_listing::{RepositoryId, RepositoryName, RepositoryOwner, RepositorySummary};
    /// let repository = RepositorySummary::new(
    ///     RepositoryId::new(1)?,
    ///     RepositoryOwner::new("octocat")?,
    ///     RepositoryName::new("hello-world")?,
    /// )
    /// .with_details(true, Some("trunk".to_owned()));
    ///
    /// assert!(repository.is_private());
    /// assert_eq!(repository.default_branch(), Some("trunk"));
    /// # Ok::<(), repovec_core::repository_listing::RepositoryListingError>(())
    /// ```
    #[must_use]
    pub fn with_details(mut self, is_private: bool, default_branch: Option<String>) -> Self {
        self.is_private = is_private;
        self.default_branch = default_branch;
        self
    }

    /// Returns the numeric repository identifier.
    ///
    /// # Examples
    ///
    /// ```
    /// # use repovec_core::repository_listing::{RepositoryId, RepositoryName, RepositoryOwner, RepositorySummary};
    /// let repository = RepositorySummary::new(
    ///     RepositoryId::new(1)?,
    ///     RepositoryOwner::new("octocat")?,
    ///     RepositoryName::new("hello-world")?,
    /// );
    /// assert_eq!(repository.id().get(), 1);
    /// # Ok::<(), repovec_core::repository_listing::RepositoryListingError>(())
    /// ```
    #[must_use]
    pub const fn id(&self) -> RepositoryId { self.id }

    /// Returns the owner login.
    ///
    /// # Examples
    ///
    /// ```
    /// # use repovec_core::repository_listing::{RepositoryId, RepositoryName, RepositoryOwner, RepositorySummary};
    /// let repository = RepositorySummary::new(
    ///     RepositoryId::new(1)?,
    ///     RepositoryOwner::new("octocat")?,
    ///     RepositoryName::new("hello-world")?,
    /// );
    /// assert_eq!(repository.owner().as_str(), "octocat");
    /// # Ok::<(), repovec_core::repository_listing::RepositoryListingError>(())
    /// ```
    #[must_use]
    pub const fn owner(&self) -> &RepositoryOwner { &self.owner }

    /// Returns the repository name.
    ///
    /// # Examples
    ///
    /// ```
    /// # use repovec_core::repository_listing::{RepositoryId, RepositoryName, RepositoryOwner, RepositorySummary};
    /// let repository = RepositorySummary::new(
    ///     RepositoryId::new(1)?,
    ///     RepositoryOwner::new("octocat")?,
    ///     RepositoryName::new("hello-world")?,
    /// );
    /// assert_eq!(repository.name().as_str(), "hello-world");
    /// # Ok::<(), repovec_core::repository_listing::RepositoryListingError>(())
    /// ```
    #[must_use]
    pub const fn name(&self) -> &RepositoryName { &self.name }

    /// Returns whether GitHub reports the repository as private.
    ///
    /// # Examples
    ///
    /// ```
    /// # use repovec_core::repository_listing::{RepositoryId, RepositoryName, RepositoryOwner, RepositorySummary};
    /// let repository = RepositorySummary::new(
    ///     RepositoryId::new(1)?,
    ///     RepositoryOwner::new("octocat")?,
    ///     RepositoryName::new("hello-world")?,
    /// )
    /// .with_details(true, None);
    /// assert!(repository.is_private());
    /// # Ok::<(), repovec_core::repository_listing::RepositoryListingError>(())
    /// ```
    #[must_use]
    pub const fn is_private(&self) -> bool { self.is_private }

    /// Returns the default branch name, absent for a repository with no commits.
    ///
    /// # Examples
    ///
    /// ```
    /// # use repovec_core::repository_listing::{RepositoryId, RepositoryName, RepositoryOwner, RepositorySummary};
    /// let repository = RepositorySummary::new(
    ///     RepositoryId::new(1)?,
    ///     RepositoryOwner::new("octocat")?,
    ///     RepositoryName::new("hello-world")?,
    /// )
    /// .with_details(false, Some("main".to_owned()));
    /// assert_eq!(repository.default_branch(), Some("main"));
    /// # Ok::<(), repovec_core::repository_listing::RepositoryListingError>(())
    /// ```
    #[must_use]
    pub fn default_branch(&self) -> Option<&str> { self.default_branch.as_deref() }
}

impl fmt::Display for RepositorySummary {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}/{}", self.owner, self.name)
    }
}
