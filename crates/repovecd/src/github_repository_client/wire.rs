//! Wire records decoded from GitHub repository listing responses.
//!
//! The records mirror only the fields this feature uses. Unknown fields are
//! tolerated because GitHub adds them routinely; a missing or wrongly typed
//! field this feature requires fails the whole listing rather than dropping a
//! repository in silence. Nothing here filters by owner type, fork status,
//! archive state, or privacy.

use repovec_core::repository_listing::{
    RepositoryId,
    RepositoryListingError,
    RepositoryName,
    RepositoryOwner,
    RepositorySummary,
};
use serde::Deserialize;

/// One repository as GitHub reports it.
#[derive(Debug, Deserialize)]
pub(crate) struct RepositoryWire {
    /// Numeric repository identifier.
    id: u64,
    /// Repository name, which is metadata rather than a filesystem path.
    name: String,
    /// Whether GitHub reports the repository as private.
    private: bool,
    /// Default branch name, absent for a repository with no commits.
    default_branch: Option<String>,
    /// Owner as GitHub reports it.
    owner: OwnerWire,
}

/// The owner subset this feature reads.
#[derive(Debug, Deserialize)]
pub(crate) struct OwnerWire {
    /// Owner login.
    login: String,
}

impl RepositoryWire {
    /// Validates the wire record into a domain record.
    ///
    /// # Errors
    ///
    /// Returns [`RepositoryListingError::InvalidResponse`] when a required
    /// value is missing or outside the domain's accepted shape.
    pub(crate) fn into_domain(self) -> Result<RepositorySummary, RepositoryListingError> {
        let id = RepositoryId::new(self.id)
            .map_err(|_| malformed("repository identifier must be positive"))?;
        let owner = RepositoryOwner::new(self.owner.login)
            .map_err(|_| malformed("repository owner login is invalid"))?;
        let name =
            RepositoryName::new(self.name).map_err(|_| malformed("repository name is invalid"))?;
        Ok(RepositorySummary::new(id, owner, name).with_details(self.private, self.default_branch))
    }
}

/// Builds the typed failure for a record outside the domain's shape.
const fn malformed(reason: &'static str) -> RepositoryListingError {
    RepositoryListingError::InvalidResponse { reason }
}
