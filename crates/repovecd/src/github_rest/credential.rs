//! Credentials a GitHub REST listing may present.
//!
//! The kind of principal is supplied explicitly. It is never inferred from a
//! token prefix, an OAuth scope string, or the shape of an HTTP error, because
//! none of those is evidence of what a credential is.

use std::fmt;

use repovec_core::{github_oauth::AccessToken, repository_listing::RepositoryListingError};

/// The principal kind a listing authenticates as.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PrincipalKind {
    /// An authenticated GitHub user.
    User,
    /// One GitHub App installation.
    Installation,
}

impl PrincipalKind {
    /// Returns the stable slug used in diagnostics.
    ///
    /// # Examples
    ///
    /// ```
    /// use repovecd::github_repository_client::PrincipalKind;
    ///
    /// assert_eq!(PrincipalKind::User.as_str(), "user");
    /// assert_eq!(PrincipalKind::Installation.as_str(), "installation");
    /// ```
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::User => "user",
            Self::Installation => "installation",
        }
    }
}

/// A GitHub App installation access token that redacts itself.
///
/// The token is already issued; minting and renewing one is out of scope. It
/// carries no OAuth scopes, because an installation token has none to report.
#[derive(Clone, Eq, PartialEq)]
pub struct InstallationAccessToken {
    /// Secret bearer credential retained for API requests only.
    value: String,
}

impl InstallationAccessToken {
    /// Creates an installation token after checking it can be sent verbatim.
    ///
    /// # Errors
    ///
    /// Returns [`RepositoryListingError::InvalidInput`] when the value is
    /// empty, contains a control character, or contains whitespace. Each of
    /// those would either send no credential or corrupt the request header.
    ///
    /// # Examples
    ///
    /// ```
    /// use repovecd::github_repository_client::InstallationAccessToken;
    ///
    /// let token = InstallationAccessToken::new("ghs_installation")?;
    ///
    /// assert_eq!(format!("{token:?}"), "InstallationAccessToken(<redacted>)");
    ///
    /// # Ok::<(), repovec_core::repository_listing::RepositoryListingError>(())
    /// ```
    pub fn new(value: impl Into<String>) -> Result<Self, RepositoryListingError> {
        let secret = value.into();
        validate_bearer_value(&secret, "installation access token")?;
        Ok(Self { value: secret })
    }

    /// Returns the secret for request construction only.
    pub(crate) fn secret(&self) -> &str { &self.value }
}

impl fmt::Debug for InstallationAccessToken {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("InstallationAccessToken(<redacted>)")
    }
}

/// The credential a listing authenticates with.
#[derive(Clone, Debug)]
pub enum GitHubCredential {
    /// An OAuth access token belonging to an authenticated user.
    User(AccessToken),
    /// An already-issued GitHub App installation access token.
    Installation(InstallationAccessToken),
}

impl GitHubCredential {
    /// Returns the principal kind this credential authenticates as.
    ///
    /// # Examples
    ///
    /// ```
    /// use repovec_core::github_oauth::AccessToken;
    /// use repovecd::github_repository_client::{GitHubCredential, PrincipalKind};
    ///
    /// let credential = GitHubCredential::User(AccessToken::new("gho_token", ["repo"]));
    ///
    /// assert_eq!(credential.principal(), PrincipalKind::User);
    /// ```
    #[must_use]
    pub const fn principal(&self) -> PrincipalKind {
        match self {
            Self::User(_) => PrincipalKind::User,
            Self::Installation(_) => PrincipalKind::Installation,
        }
    }

    /// Returns the secret for request construction only.
    pub(crate) fn secret(&self) -> &str {
        match self {
            Self::User(token) => token.secret(),
            Self::Installation(token) => token.secret(),
        }
    }

    /// Checks the secret can be sent as a bearer credential.
    ///
    /// A stored user token never passed through [`InstallationAccessToken`],
    /// so it is checked here before the first request.
    pub(crate) fn validate(&self) -> Result<(), RepositoryListingError> {
        let field = match self {
            Self::User(_) => "user access token",
            Self::Installation(_) => "installation access token",
        };
        validate_bearer_value(self.secret(), field)
    }
}

/// Rejects values that would send no credential or corrupt a header.
fn validate_bearer_value(value: &str, field: &'static str) -> Result<(), RepositoryListingError> {
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
    if value.chars().any(char::is_whitespace) {
        return Err(RepositoryListingError::InvalidInput {
            field,
            reason: "must not contain whitespace",
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    //! Unit tests for credential validation and redaction.

    use repovec_core::github_oauth::AccessToken;
    use rstest::rstest;

    use super::{GitHubCredential, InstallationAccessToken, PrincipalKind};

    #[test]
    fn installation_tokens_redact_their_secret_in_debug_output() {
        let token = InstallationAccessToken::new("ghs_secret_value")
            .expect("a header-safe installation token should be accepted");
        let rendered = format!("{token:?}");

        assert_eq!(
            rendered, "InstallationAccessToken(<redacted>)",
            "installation token debug output must not reveal the secret"
        );
    }

    #[test]
    fn credentials_redact_their_secret_in_debug_output() {
        let user = GitHubCredential::User(AccessToken::new("gho_secret_value", ["repo"]));
        let installation = GitHubCredential::Installation(
            InstallationAccessToken::new("ghs_secret_value")
                .expect("a header-safe installation token should be accepted"),
        );

        for credential in [user, installation] {
            let rendered = format!("{credential:?}");
            assert!(
                !rendered.contains("secret_value"),
                "credential debug output must not reveal the secret: {rendered}"
            );
        }
    }

    #[test]
    fn the_principal_kind_is_supplied_not_inferred() {
        let user = GitHubCredential::User(AccessToken::new(
            "ghs_looks_installational",
            Vec::<String>::new(),
        ));
        let installation = GitHubCredential::Installation(
            InstallationAccessToken::new("gho_looks_userish")
                .expect("a header-safe installation token should be accepted"),
        );

        assert_eq!(
            user.principal(),
            PrincipalKind::User,
            "a user credential must stay a user credential whatever its prefix"
        );
        assert_eq!(
            installation.principal(),
            PrincipalKind::Installation,
            "an installation credential must stay an installation credential"
        );
    }

    #[rstest]
    #[case::empty("", "must not be empty")]
    #[case::leading_space(" ghs_token", "must not contain whitespace")]
    #[case::trailing_space("ghs_token ", "must not contain whitespace")]
    #[case::interior_space("ghs token", "must not contain whitespace")]
    #[case::newline("ghs\ntoken", "must not contain control characters")]
    #[case::nul("ghs\0token", "must not contain control characters")]
    fn unsafe_installation_tokens_are_rejected(#[case] value: &str, #[case] reason: &'static str) {
        let error = InstallationAccessToken::new(value)
            .expect_err("a value unsafe for a request header must be rejected");

        assert_eq!(
            error,
            repovec_core::repository_listing::RepositoryListingError::InvalidInput {
                field: "installation access token",
                reason,
            },
            "unexpected rejection reason for {value:?}"
        );
    }

    #[test]
    fn a_stored_user_token_is_validated_before_use() {
        let credential = GitHubCredential::User(AccessToken::new("", ["repo"]));

        assert!(
            credential.validate().is_err(),
            "an empty stored user token must be rejected before a request"
        );
    }
}
