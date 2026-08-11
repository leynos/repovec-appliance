//! Semantic validation errors for the repovec directory-layout contract.
//!
//! The parent `directory_layout` module returns these errors from its
//! checked-in and caller-provided validation functions so callers can
//! distinguish parse, missing-entry, and ownership failures without inspecting
//! display strings.

use std::{error::Error, fmt};

use camino::Utf8PathBuf;

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
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DirectoryLayoutError {
    /// A non-comment, non-blank asset line could not be parsed.
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
    NonExplicitField {
        /// The logical asset name.
        asset: &'static str,
        /// The 1-indexed source line number.
        line_number: usize,
        /// The field name that must be explicit.
        field: &'static str,
    },
    /// A required directory entry is absent from the `tmpfiles.d` asset.
    MissingDirectoryEntry {
        /// The required directory path.
        path: Utf8PathBuf,
    },
    /// A directory entry not declared by the contract appears in the asset.
    UnexpectedDirectoryEntry {
        /// The unexpected directory path.
        path: Utf8PathBuf,
    },
    /// The `tmpfiles.d` asset declares the secrets directory.
    ForbiddenSecretsEntry {
        /// The forbidden directory path.
        path: Utf8PathBuf,
    },
    /// A directory entry has a mode that differs from its contract expectation.
    IncorrectMode {
        /// The directory path.
        path: Utf8PathBuf,
        /// The expected octal mode.
        expected: Mode,
        /// The observed octal mode.
        actual: Mode,
    },
    /// A directory entry has an owner that differs from its contract.
    IncorrectOwner {
        /// The directory path.
        path: Utf8PathBuf,
        /// The expected owner name.
        expected: &'static str,
        /// The observed owner name.
        actual: String,
    },
    /// A directory entry has a group that differs from its contract.
    IncorrectGroup {
        /// The directory path.
        path: Utf8PathBuf,
        /// The expected group name.
        expected: &'static str,
        /// The observed group name.
        actual: String,
    },
    /// The `sysusers.d` asset does not declare the `repovec` user.
    SysusersMissingUser,
    /// The `sysusers.d` asset declares a home directory other than the expected.
    SysusersIncorrectHome {
        /// The expected home directory.
        expected: &'static str,
        /// The observed home directory.
        actual: String,
    },
    /// The `sysusers.d` asset declares a shell other than the expected.
    SysusersIncorrectShell {
        /// The expected shell.
        expected: &'static str,
        /// The observed shell.
        actual: String,
    },
}

impl DirectoryLayoutError {
    /// Returns the packaging asset the failure relates to, for structured logging.
    #[must_use]
    pub const fn asset(&self) -> &'static str {
        match self {
            Self::MalformedLine { asset, .. } | Self::NonExplicitField { asset, .. } => asset,
            Self::MissingDirectoryEntry { .. }
            | Self::UnexpectedDirectoryEntry { .. }
            | Self::ForbiddenSecretsEntry { .. }
            | Self::IncorrectMode { .. }
            | Self::IncorrectOwner { .. }
            | Self::IncorrectGroup { .. } => "tmpfiles.d/repovec.conf",
            Self::SysusersMissingUser
            | Self::SysusersIncorrectHome { .. }
            | Self::SysusersIncorrectShell { .. } => "sysusers.d/repovec.conf",
        }
    }
}

impl fmt::Display for DirectoryLayoutError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MalformedLine { asset, line_number, line } => {
                write!(f, "malformed line in {asset} at {line_number}: {line}")
            }
            Self::NonExplicitField { asset, line_number, field } => {
                write!(f, "{asset} must set {field} explicitly on line {line_number}")
            }
            Self::MissingDirectoryEntry { path } => {
                write!(f, "missing directory entry for {path}")
            }
            Self::UnexpectedDirectoryEntry { path } => {
                write!(f, "unexpected directory entry for {path}")
            }
            Self::ForbiddenSecretsEntry { path } => {
                write!(f, "{path} must not be declared by the tmpfiles.d asset")
            }
            Self::IncorrectMode { path, expected, actual } => {
                write!(f, "{path} must have mode {expected}: {actual}")
            }
            Self::IncorrectOwner { path, expected, actual } => {
                write!(f, "{path} must be owned by {expected}: {actual}")
            }
            Self::IncorrectGroup { path, expected, actual } => {
                write!(f, "{path} must be in group {expected}: {actual}")
            }
            Self::SysusersMissingUser => {
                write!(f, "the sysusers.d asset must declare the repovec user")
            }
            Self::SysusersIncorrectHome { expected, actual } => {
                write!(f, "the repovec user home must be {expected}: {actual}")
            }
            Self::SysusersIncorrectShell { expected, actual } => {
                write!(f, "the repovec user shell must be {expected}: {actual}")
            }
        }
    }
}

impl Error for DirectoryLayoutError {}

#[cfg(test)]
mod tests {
    //! Unit coverage for directory-layout validation errors.

    use camino::Utf8PathBuf;

    use super::{DirectoryLayoutError, Mode};

    #[test]
    fn asset_returns_logical_asset_name() {
        let cases = [
            DirectoryLayoutError::MalformedLine {
                asset: "tmpfiles.d/repovec.conf",
                line_number: 1,
                line: "bogus".to_owned(),
            },
            DirectoryLayoutError::NonExplicitField {
                asset: "sysusers.d/repovec.conf",
                line_number: 1,
                field: "mode",
            },
            DirectoryLayoutError::MissingDirectoryEntry {
                path: Utf8PathBuf::from("/var/lib/repovec/worktrees"),
            },
            DirectoryLayoutError::UnexpectedDirectoryEntry {
                path: Utf8PathBuf::from("/var/lib/repovec/other"),
            },
            DirectoryLayoutError::ForbiddenSecretsEntry { path: Utf8PathBuf::from("/etc/repovec") },
            DirectoryLayoutError::IncorrectMode {
                path: Utf8PathBuf::from("/var/lib/repovec"),
                expected: Mode(0o700),
                actual: Mode(0o750),
            },
            DirectoryLayoutError::IncorrectOwner {
                path: Utf8PathBuf::from("/var/lib/repovec"),
                expected: "repovec",
                actual: "root".to_owned(),
            },
            DirectoryLayoutError::IncorrectGroup {
                path: Utf8PathBuf::from("/var/lib/repovec"),
                expected: "repovec",
                actual: "wheel".to_owned(),
            },
            DirectoryLayoutError::SysusersMissingUser,
            DirectoryLayoutError::SysusersIncorrectHome {
                expected: "/var/lib/repovec",
                actual: "/home/repovec".to_owned(),
            },
            DirectoryLayoutError::SysusersIncorrectShell {
                expected: "/usr/sbin/nologin",
                actual: "/bin/bash".to_owned(),
            },
        ];

        let observed = cases.iter().map(DirectoryLayoutError::asset).collect::<Vec<_>>();

        assert_eq!(
            observed,
            [
                "tmpfiles.d/repovec.conf",
                "sysusers.d/repovec.conf",
                "tmpfiles.d/repovec.conf",
                "tmpfiles.d/repovec.conf",
                "tmpfiles.d/repovec.conf",
                "tmpfiles.d/repovec.conf",
                "tmpfiles.d/repovec.conf",
                "tmpfiles.d/repovec.conf",
                "sysusers.d/repovec.conf",
                "sysusers.d/repovec.conf",
                "sysusers.d/repovec.conf",
            ],
        );
    }
}
