//! Fixture builders for repository listing tests.
//!
//! Each builder returns a `Result` so a test can propagate an invalid fixture
//! with `?` instead of panicking, which keeps the shared helpers free of
//! `expect` outside `#[cfg(test)]` modules. [`RepositoryFixture`] describes one
//! repository and [`numbered_repositories`] produces an ordered run of them.

use repovec_core::repository_listing::{
    RepositoryId,
    RepositoryListingError,
    RepositoryName,
    RepositoryOwner,
    RepositorySummary,
};

/// A repository fixture under construction.
///
/// # Examples
///
/// ```
/// use repovec_test_helpers::repository_listing::RepositoryFixture;
///
/// let repository = RepositoryFixture::private(2, "acme", "private-svc").build()?;
///
/// assert!(repository.is_private());
/// assert_eq!(repository.default_branch(), None);
///
/// # Ok::<(), repovec_core::repository_listing::RepositoryListingError>(())
/// ```
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RepositoryFixture<'a> {
    /// Numeric repository identifier.
    pub id: u64,
    /// Owner login.
    pub owner: &'a str,
    /// Repository name.
    pub name: &'a str,
    /// Whether GitHub reports the repository as private.
    pub is_private: bool,
    /// Default branch name, absent for a repository with no commits.
    pub default_branch: Option<&'a str>,
}

impl<'a> RepositoryFixture<'a> {
    /// Starts a public fixture whose default branch is `main`.
    ///
    /// # Examples
    ///
    /// ```
    /// use repovec_test_helpers::repository_listing::RepositoryFixture;
    ///
    /// let repository = RepositoryFixture::public(1, "octocat", "hello-world").build()?;
    ///
    /// assert_eq!(repository.to_string(), "octocat/hello-world");
    /// assert_eq!(repository.default_branch(), Some("main"));
    ///
    /// # Ok::<(), repovec_core::repository_listing::RepositoryListingError>(())
    /// ```
    #[must_use]
    pub const fn public(id: u64, owner: &'a str, name: &'a str) -> Self {
        Self {
            id,
            owner,
            name,
            is_private: false,
            default_branch: Some("main"),
        }
    }

    /// Starts a private fixture with no default branch.
    ///
    /// # Examples
    ///
    /// ```
    /// use repovec_test_helpers::repository_listing::RepositoryFixture;
    ///
    /// let fixture = RepositoryFixture::private(2, "acme", "private-svc");
    ///
    /// assert!(fixture.is_private);
    /// assert_eq!(fixture.default_branch, None);
    /// ```
    #[must_use]
    pub const fn private(id: u64, owner: &'a str, name: &'a str) -> Self {
        Self {
            id,
            owner,
            name,
            is_private: true,
            default_branch: None,
        }
    }

    /// Sets the default branch name.
    ///
    /// # Examples
    ///
    /// ```
    /// use repovec_test_helpers::repository_listing::RepositoryFixture;
    ///
    /// let repository = RepositoryFixture::private(2, "acme", "svc")
    ///     .with_default_branch("trunk")
    ///     .build()?;
    ///
    /// assert_eq!(repository.default_branch(), Some("trunk"));
    ///
    /// # Ok::<(), repovec_core::repository_listing::RepositoryListingError>(())
    /// ```
    #[must_use]
    pub const fn with_default_branch(mut self, branch: &'a str) -> Self {
        self.default_branch = Some(branch);
        self
    }

    /// Builds the validated repository summary.
    ///
    /// # Errors
    ///
    /// Returns [`RepositoryListingError::InvalidInput`] when the identifier,
    /// owner login, or repository name is not valid.
    ///
    /// # Examples
    ///
    /// ```
    /// use repovec_test_helpers::repository_listing::RepositoryFixture;
    ///
    /// assert!(
    ///     RepositoryFixture::public(0, "octocat", "hello")
    ///         .build()
    ///         .is_err()
    /// );
    /// assert!(
    ///     RepositoryFixture::public(1, "octocat", "hello")
    ///         .build()
    ///         .is_ok()
    /// );
    /// ```
    pub fn build(self) -> Result<RepositorySummary, RepositoryListingError> {
        Ok(RepositorySummary::new(
            RepositoryId::new(self.id)?,
            RepositoryOwner::new(self.owner)?,
            RepositoryName::new(self.name)?,
        )
        .with_details(self.is_private, self.default_branch.map(ToOwned::to_owned)))
    }
}

/// Builds a public repository summary with the default branch `main`.
///
/// # Errors
///
/// Returns [`RepositoryListingError::InvalidInput`] when any supplied value is
/// not a valid identifier.
///
/// # Examples
///
/// ```
/// use repovec_test_helpers::repository_listing::repository_summary;
///
/// let repository = repository_summary(1, "octocat", "hello-world")?;
///
/// assert_eq!(repository.to_string(), "octocat/hello-world");
/// assert!(!repository.is_private());
/// assert_eq!(repository.default_branch(), Some("main"));
///
/// # Ok::<(), repovec_core::repository_listing::RepositoryListingError>(())
/// ```
pub fn repository_summary(
    id: u64,
    owner: &str,
    name: &str,
) -> Result<RepositorySummary, RepositoryListingError> {
    RepositoryFixture::public(id, owner, name).build()
}

/// Builds `count` public repository summaries numbered from one under one owner.
///
/// # Errors
///
/// Returns [`RepositoryListingError::InvalidInput`] when the owner login is
/// invalid.
///
/// # Examples
///
/// ```
/// use repovec_test_helpers::repository_listing::numbered_repositories;
///
/// let repositories = numbered_repositories("octocat", 3)?;
///
/// assert_eq!(repositories.len(), 3);
/// assert_eq!(repositories[2].name().as_str(), "repo-3");
///
/// # Ok::<(), repovec_core::repository_listing::RepositoryListingError>(())
/// ```
pub fn numbered_repositories(
    owner: &str,
    count: u32,
) -> Result<Vec<RepositorySummary>, RepositoryListingError> {
    (1..=count)
        .map(|index| repository_summary(u64::from(index), owner, &format!("repo-{index}")))
        .collect()
}
