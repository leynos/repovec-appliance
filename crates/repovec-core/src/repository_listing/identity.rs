//! Validated repository identity components.
//!
//! An owner login and a repository name are identifiers reported by GitHub,
//! never filesystem paths. Construction is fallible so that an empty or
//! control-character-bearing identifier cannot reach a caller or a
//! line-oriented renderer.

use std::fmt;

use super::RepositoryListingError;

/// Positive numeric GitHub repository identifier.
///
/// # Examples
///
/// ```
/// use repovec_core::repository_listing::RepositoryId;
///
/// let id = RepositoryId::new(42)?;
///
/// assert_eq!(id.get(), 42, "the identifier should be kept");
/// assert!(RepositoryId::new(0).is_err(), "zero must be refused");
///
/// # Ok::<(), repovec_core::repository_listing::RepositoryListingError>(())
/// ```
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct RepositoryId(u64);

impl RepositoryId {
    /// Creates a repository identifier, rejecting zero.
    ///
    /// # Parameters
    /// - `value`: Candidate identifier reported by GitHub.
    ///
    /// # Returns
    /// The identifier, when `value` is a positive number.
    ///
    /// # Errors
    ///
    /// Returns [`RepositoryListingError::InvalidInput`] when `value` is zero,
    /// because GitHub never issues a zero repository identifier.
    ///
    /// # Examples
    ///
    /// ```
    /// use repovec_core::repository_listing::RepositoryId;
    ///
    /// assert_eq!(
    ///     RepositoryId::new(1)?.get(),
    ///     1,
    ///     "a positive identifier should be kept"
    /// );
    /// assert!(RepositoryId::new(0).is_err(), "zero must be refused");
    ///
    /// # Ok::<(), repovec_core::repository_listing::RepositoryListingError>(())
    /// ```
    pub const fn new(value: u64) -> Result<Self, RepositoryListingError> {
        if value == 0 {
            return Err(RepositoryListingError::InvalidInput {
                field: "repository id",
                reason: "must be a positive number",
            });
        }
        Ok(Self(value))
    }

    /// Returns the numeric identifier.
    ///
    /// # Returns
    /// The identifier value GitHub reported.
    ///
    /// # Examples
    ///
    /// ```
    /// use repovec_core::repository_listing::RepositoryId;
    ///
    /// assert_eq!(
    ///     RepositoryId::new(7)?.get(),
    ///     7,
    ///     "the accessor should return the identifier"
    /// );
    ///
    /// # Ok::<(), repovec_core::repository_listing::RepositoryListingError>(())
    /// ```
    #[must_use]
    pub const fn get(self) -> u64 { self.0 }
}

impl fmt::Display for RepositoryId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { self.0.fmt(f) }
}

/// Account login that owns a repository.
///
/// # Examples
///
/// ```
/// use repovec_core::repository_listing::RepositoryOwner;
///
/// assert_eq!(
///     RepositoryOwner::new("octocat")?.as_str(),
///     "octocat",
///     "the login should be kept"
/// );
/// assert!(
///     RepositoryOwner::new("").is_err(),
///     "an empty login must be refused"
/// );
///
/// # Ok::<(), repovec_core::repository_listing::RepositoryListingError>(())
/// ```
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct RepositoryOwner(String);

impl RepositoryOwner {
    /// Creates an owner login, rejecting empty or control-bearing values.
    ///
    /// # Parameters
    /// - `value`: Candidate account login reported by GitHub.
    ///
    /// # Returns
    /// The validated login.
    ///
    /// # Errors
    ///
    /// Returns [`RepositoryListingError::InvalidInput`] when `value` is empty
    /// or contains a control character that would corrupt line-oriented output.
    ///
    /// # Examples
    ///
    /// ```
    /// use repovec_core::repository_listing::RepositoryOwner;
    ///
    /// assert_eq!(
    ///     RepositoryOwner::new("octocat")?.as_str(),
    ///     "octocat",
    ///     "the constructor should keep the login"
    /// );
    /// assert!(
    ///     RepositoryOwner::new("").is_err(),
    ///     "an empty login must be refused"
    /// );
    ///
    /// # Ok::<(), repovec_core::repository_listing::RepositoryListingError>(())
    /// ```
    pub fn new(value: impl Into<String>) -> Result<Self, RepositoryListingError> {
        let login = value.into();
        validate_metadata_name(&login, "repository owner login")?;
        Ok(Self(login))
    }

    /// Returns the owner login as reported by GitHub.
    ///
    /// # Returns
    /// The account login.
    ///
    /// # Examples
    ///
    /// ```
    /// use repovec_core::repository_listing::RepositoryOwner;
    ///
    /// assert_eq!(
    ///     RepositoryOwner::new("octocat")?.as_str(),
    ///     "octocat",
    ///     "the accessor should return the login"
    /// );
    ///
    /// # Ok::<(), repovec_core::repository_listing::RepositoryListingError>(())
    /// ```
    #[must_use]
    pub fn as_str(&self) -> &str { &self.0 }
}

impl fmt::Display for RepositoryOwner {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { f.write_str(&self.0) }
}

impl AsRef<str> for RepositoryOwner {
    fn as_ref(&self) -> &str { self.as_str() }
}

/// Repository name within its owner account.
///
/// # Examples
///
/// ```
/// use repovec_core::repository_listing::RepositoryName;
///
/// assert_eq!(
///     RepositoryName::new("repovec")?.as_str(),
///     "repovec",
///     "the name should be kept"
/// );
/// assert!(
///     RepositoryName::new("").is_err(),
///     "an empty name must be refused"
/// );
///
/// # Ok::<(), repovec_core::repository_listing::RepositoryListingError>(())
/// ```
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct RepositoryName(String);

impl RepositoryName {
    /// Creates a repository name, rejecting empty or control-bearing values.
    ///
    /// # Parameters
    /// - `value`: Candidate repository name reported by GitHub.
    ///
    /// # Returns
    /// The validated repository name.
    ///
    /// # Errors
    ///
    /// Returns [`RepositoryListingError::InvalidInput`] when `value` is empty
    /// or contains a control character that would corrupt line-oriented output.
    ///
    /// # Examples
    ///
    /// ```
    /// use repovec_core::repository_listing::RepositoryName;
    ///
    /// assert_eq!(
    ///     RepositoryName::new("repovec")?.as_str(),
    ///     "repovec",
    ///     "the constructor should keep the name"
    /// );
    /// assert!(
    ///     RepositoryName::new("").is_err(),
    ///     "an empty name must be refused"
    /// );
    ///
    /// # Ok::<(), repovec_core::repository_listing::RepositoryListingError>(())
    /// ```
    pub fn new(value: impl Into<String>) -> Result<Self, RepositoryListingError> {
        let name = value.into();
        validate_metadata_name(&name, "repository name")?;
        Ok(Self(name))
    }

    /// Returns the repository name as reported by GitHub.
    ///
    /// # Returns
    /// The repository name.
    ///
    /// # Examples
    ///
    /// ```
    /// use repovec_core::repository_listing::RepositoryName;
    ///
    /// assert_eq!(
    ///     RepositoryName::new("repovec")?.as_str(),
    ///     "repovec",
    ///     "the accessor should return the name"
    /// );
    ///
    /// # Ok::<(), repovec_core::repository_listing::RepositoryListingError>(())
    /// ```
    #[must_use]
    pub fn as_str(&self) -> &str { &self.0 }
}

impl fmt::Display for RepositoryName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { f.write_str(&self.0) }
}

impl AsRef<str> for RepositoryName {
    fn as_ref(&self) -> &str { self.as_str() }
}

/// Rejects an empty metadata identifier or one containing control characters.
///
/// A control character in an owner login or repository name would corrupt the
/// line-oriented diagnostic output the listing renders, and GitHub does not
/// issue such identifiers, so a wire record carrying one is malformed.
fn validate_metadata_name(value: &str, field: &'static str) -> Result<(), RepositoryListingError> {
    if value.is_empty() {
        return Err(RepositoryListingError::InvalidInput {
            field,
            reason: "must not be empty",
        });
    }
    if value.chars().any(char::is_control) {
        return Err(RepositoryListingError::InvalidInput {
            field,
            reason: "must not contain control characters",
        });
    }
    Ok(())
}
