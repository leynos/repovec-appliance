//! Semantic validation errors for the repovec directory-layout contract.
//!
//! The parent `directory_layout` module returns these errors from its
//! checked-in and caller-provided validation functions so callers can
//! distinguish parse, missing-entry, and ownership failures without inspecting
//! display strings.

use std::fmt;

use camino::Utf8PathBuf;

/// The logical name of the `tmpfiles.d` asset in diagnostics.
pub(crate) const TMPFILES_ASSET: &str = "tmpfiles.d/repovec.conf";
/// The logical name of the `sysusers.d` asset in diagnostics.
pub(crate) const SYSUSERS_ASSET: &str = "sysusers.d/repovec.conf";

/// A POSIX directory mode expressed as an octal mode value.
///
/// `Display` renders the value as a zero-padded four-digit octal mode so
/// operator-facing messages match the `tmpfiles.d` asset spelling (for example
/// `0700`).
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Mode(pub u16);

impl fmt::Display for Mode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { write!(f, "{:04o}", self.0) }
}

/// Contract failures for the repovec directory-layout assets.
#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
pub enum DirectoryLayoutError {
    /// A non-comment, non-blank asset line could not be parsed.
    #[error("malformed line in {asset} at {line_number}: {line}")]
    MalformedLine {
        /// The logical asset name (`"tmpfiles.d/repovec.conf"` or
        /// `"sysusers.d/repovec.conf"`).
        asset: &'static str,
        /// The 1-indexed source line number.
        line_number: usize,
        /// The invalid line contents after trimming.
        line: String,
    },
    /// A managed field used the `-` default token instead of an explicit value.
    #[error("{asset} must set {field} explicitly on line {line_number}")]
    NonExplicitField {
        /// The logical asset name.
        asset: &'static str,
        /// The 1-indexed source line number.
        line_number: usize,
        /// The field name that must be explicit.
        field: &'static str,
    },
    /// A managed field is present but is not a valid value.
    ///
    /// Distinct from [`Self::NonExplicitField`]: the field *is* explicit, so
    /// telling the operator to set it explicitly would point at the wrong fix.
    #[error("{asset} has an invalid {field} on line {line_number}: {value}")]
    InvalidField {
        /// The logical asset name.
        asset: &'static str,
        /// The 1-indexed source line number.
        line_number: usize,
        /// The field name that is invalid.
        field: &'static str,
        /// The invalid value as written.
        value: String,
    },
    /// A `sysusers.d` `u` line is missing a column this validator requires.
    #[error("{asset} must declare the {field} column on line {line_number}")]
    MissingSysusersField {
        /// The logical asset name.
        asset: &'static str,
        /// The 1-indexed source line number.
        line_number: usize,
        /// The first column absent from the line.
        field: &'static str,
    },
    /// A required directory entry is absent from the `tmpfiles.d` asset.
    #[error("missing directory entry for {path}")]
    MissingDirectoryEntry {
        /// The required directory path.
        path: Utf8PathBuf,
    },
    /// A directory entry not declared by the contract appears in the asset.
    #[error("unexpected directory entry for {path}")]
    UnexpectedDirectoryEntry {
        /// The unexpected directory path.
        path: Utf8PathBuf,
    },
    /// The `tmpfiles.d` asset declares the secrets directory.
    #[error("{path} must not be declared by the {TMPFILES_ASSET} asset")]
    ForbiddenSecretsEntry {
        /// The forbidden directory path.
        path: Utf8PathBuf,
    },
    /// A directory entry has a mode that differs from its contract expectation.
    #[error("{path} must have mode {expected}: {actual}")]
    IncorrectMode {
        /// The directory path.
        path: Utf8PathBuf,
        /// The expected octal mode.
        expected: Mode,
        /// The observed octal mode.
        actual: Mode,
    },
    /// A directory entry has an owner that differs from its contract.
    #[error("{path} must be owned by {expected}: {actual}")]
    IncorrectOwner {
        /// The directory path.
        path: Utf8PathBuf,
        /// The expected owner name.
        expected: &'static str,
        /// The observed owner name.
        actual: String,
    },
    /// A directory entry has a group that differs from its contract.
    #[error("{path} must be in group {expected}: {actual}")]
    IncorrectGroup {
        /// The directory path.
        path: Utf8PathBuf,
        /// The expected group name.
        expected: &'static str,
        /// The observed group name.
        actual: String,
    },
    /// The `sysusers.d` asset does not declare the `repovec` user.
    #[error("the {SYSUSERS_ASSET} asset must declare the repovec user")]
    SysusersMissingUser,
    /// The `sysusers.d` asset pins a numeric uid instead of the allocation token.
    #[error("the repovec user uid must be allocated by systemd-sysusers, not pinned: {actual}")]
    SysusersPinnedUid {
        /// The uid as written in the asset.
        actual: String,
    },
    /// The `sysusers.d` asset declares a home directory other than the expected.
    #[error("the repovec user home must be {expected}: {actual}")]
    SysusersIncorrectHome {
        /// The expected home directory.
        expected: &'static str,
        /// The observed home directory.
        actual: String,
    },
    /// The `sysusers.d` asset declares a shell other than the expected.
    #[error("the repovec user shell must be {expected}: {actual}")]
    SysusersIncorrectShell {
        /// The expected shell.
        expected: &'static str,
        /// The observed shell.
        actual: String,
    },
}

impl DirectoryLayoutError {
    /// Returns the packaging asset the failure relates to, for structured logging.
    ///
    /// # Examples
    ///
    /// ```
    /// use camino::Utf8PathBuf;
    /// use repovec_core::appliance::directory_layout::DirectoryLayoutError;
    ///
    /// let error = DirectoryLayoutError::MalformedLine {
    ///     asset: "sysusers.d/repovec.conf",
    ///     line_number: 3,
    ///     line: "bogus".to_owned(),
    /// };
    /// assert_eq!(error.asset(), "sysusers.d/repovec.conf");
    ///
    /// let missing = DirectoryLayoutError::MissingDirectoryEntry {
    ///     path: Utf8PathBuf::from("/var/lib/repovec/worktrees"),
    /// };
    /// assert_eq!(missing.asset(), "tmpfiles.d/repovec.conf");
    /// ```
    #[must_use]
    pub const fn asset(&self) -> &'static str {
        match self {
            Self::MalformedLine { asset, .. }
            | Self::NonExplicitField { asset, .. }
            | Self::InvalidField { asset, .. }
            | Self::MissingSysusersField { asset, .. } => asset,
            Self::MissingDirectoryEntry { .. }
            | Self::UnexpectedDirectoryEntry { .. }
            | Self::ForbiddenSecretsEntry { .. }
            | Self::IncorrectMode { .. }
            | Self::IncorrectOwner { .. }
            | Self::IncorrectGroup { .. } => TMPFILES_ASSET,
            Self::SysusersMissingUser
            | Self::SysusersPinnedUid { .. }
            | Self::SysusersIncorrectHome { .. }
            | Self::SysusersIncorrectShell { .. } => SYSUSERS_ASSET,
        }
    }
}

#[cfg(test)]
mod tests {
    //! Unit coverage for directory-layout validation errors.

    use camino::Utf8PathBuf;

    use super::{DirectoryLayoutError, Mode, SYSUSERS_ASSET, TMPFILES_ASSET};

    #[rstest::rstest]
    #[case(
        DirectoryLayoutError::MalformedLine {
            asset: TMPFILES_ASSET,
            line_number: 1,
            line: "bogus".to_owned(),
        },
        TMPFILES_ASSET
    )]
    #[case(
        DirectoryLayoutError::NonExplicitField {
            asset: SYSUSERS_ASSET,
            line_number: 1,
            field: "mode",
        },
        SYSUSERS_ASSET
    )]
    #[case(
        DirectoryLayoutError::InvalidField {
            asset: TMPFILES_ASSET,
            line_number: 2,
            field: "mode",
            value: "0o900".to_owned(),
        },
        TMPFILES_ASSET
    )]
    #[case(
        DirectoryLayoutError::MissingSysusersField {
            asset: SYSUSERS_ASSET,
            line_number: 3,
            field: "uid",
        },
        SYSUSERS_ASSET
    )]
    #[case(
        DirectoryLayoutError::MissingDirectoryEntry {
            path: Utf8PathBuf::from("/var/lib/repovec/worktrees"),
        },
        TMPFILES_ASSET
    )]
    #[case(
        DirectoryLayoutError::UnexpectedDirectoryEntry {
            path: Utf8PathBuf::from("/var/lib/repovec/other"),
        },
        TMPFILES_ASSET
    )]
    #[case(
        DirectoryLayoutError::ForbiddenSecretsEntry { path: Utf8PathBuf::from("/etc/repovec") },
        TMPFILES_ASSET
    )]
    #[case(
        DirectoryLayoutError::IncorrectMode {
            path: Utf8PathBuf::from("/var/lib/repovec"),
            expected: Mode(0o700),
            actual: Mode(0o750),
        },
        TMPFILES_ASSET
    )]
    #[case(
        DirectoryLayoutError::IncorrectOwner {
            path: Utf8PathBuf::from("/var/lib/repovec"),
            expected: "repovec",
            actual: "root".to_owned(),
        },
        TMPFILES_ASSET
    )]
    #[case(
        DirectoryLayoutError::IncorrectGroup {
            path: Utf8PathBuf::from("/var/lib/repovec"),
            expected: "repovec",
            actual: "wheel".to_owned(),
        },
        TMPFILES_ASSET
    )]
    #[case(DirectoryLayoutError::SysusersMissingUser, SYSUSERS_ASSET)]
    #[case(
        DirectoryLayoutError::SysusersPinnedUid { actual: "1000".to_owned() },
        SYSUSERS_ASSET
    )]
    #[case(
        DirectoryLayoutError::SysusersIncorrectHome {
            expected: "/var/lib/repovec",
            actual: "/home/repovec".to_owned(),
        },
        SYSUSERS_ASSET
    )]
    #[case(
        DirectoryLayoutError::SysusersIncorrectShell {
            expected: "/usr/sbin/nologin",
            actual: "/bin/bash".to_owned(),
        },
        SYSUSERS_ASSET
    )]
    fn asset_returns_logical_asset_name(
        #[case] error: DirectoryLayoutError,
        #[case] expected: &'static str,
    ) {
        assert_eq!(error.asset(), expected);
    }
}
