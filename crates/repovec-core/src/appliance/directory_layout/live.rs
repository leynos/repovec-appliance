//! Live directory-layout pre-flight for daemon startup.
//!
//! This adapter is explicitly OUTSIDE the pure static validator in the parent
//! module. The pure validator parses checked-in asset text; this module stats
//! the real on-disk tree and refuses to proceed when a live host is
//! misprovisioned (Risk R-6). It is the fail-closed runtime backstop for the
//! static contract: a correct asset does not guarantee a correct host.
//!
//! The contract checked here mirrors the spec table in `mod.rs`:
//!
//! - the data tree (`/var/lib/repovec` and its `git-mirrors/`, `worktrees/`,
//!   `.grepai/` children) must be directories owned by the `repovec` user and
//!   group with mode `0700` (SI-1);
//! - `qdrant-storage` must be a directory owned `root:root` with mode `0700`
//!   (SI-2);
//! - `/etc/repovec` must be a directory owned `root`, group `repovec`, with
//!   mode `0750` (SI-3).
//!
//! Both account names are resolved through their own databases: the uid from
//! `/etc/passwd` and the gid from `/etc/group`. `systemd-tmpfiles` resolves its
//! group column through the group database, so deriving the expected group from
//! the user's primary GID would reject correctly provisioned trees on hosts
//! where the `repovec` user and group do not share a numeric id.
//!
//! The `passwd` row is parsed in full, not just for its uid. `systemd-sysusers`
//! leaves an existing account untouched, so a pre-existing root-owned or
//! login-capable `repovec` account survives provisioning; the pre-flight
//! rejects it rather than trusting the by-name ownership that
//! `systemd-tmpfiles` would apply. The record parsing and its rules live in
//! `super::account`.

use std::io;

use camino::Utf8PathBuf;

use super::directory_contract::{DirectoryContract, verify_directory};
pub use super::live_resolution::{LiveLayoutIds, verify_live_layout};
#[cfg(test)]
pub(crate) use super::live_resolution::{resolve_ids_from_databases, verify_layout_from_databases};
use crate::RuntimePaths;

/// A directory that failed the live layout pre-flight.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LiveLayoutViolation {
    /// The directory that failed the check.
    pub path: Utf8PathBuf,
    /// The expected value (mode, owner, or group) rendered for operators.
    pub expected: String,
    /// The observed value rendered for operators.
    pub actual: String,
}

/// Failures detected by the live directory-layout pre-flight.
#[derive(Debug, thiserror::Error)]
pub enum LiveLayoutError {
    /// The `repovec` account is not present in `/etc/passwd`, so ownership
    /// cannot be resolved by name.
    #[error("the repovec system user does not exist")]
    RepovecUserNotFound,
    /// The `repovec` group is not present in `/etc/group`, so ownership
    /// cannot be resolved by name.
    #[error("the repovec system group does not exist")]
    RepovecGroupNotFound,
    /// The `repovec` account exists but its uid is root.
    ///
    /// `systemd-sysusers` leaves an existing account untouched, so a host that
    /// already carried a root-owned `repovec` entry would otherwise hand the
    /// data tree, and every unit that runs as `User=repovec`, to root.
    #[error("the repovec system user must not be root: uid {uid}")]
    RepovecUserIsRoot {
        /// The root uid the account resolved to.
        uid: u32,
    },
    /// The `repovec` account exists but its home directory is not the
    /// contract home.
    ///
    /// The same sysusers idempotence rule that makes a wrong uid survive
    /// provisioning also makes a wrong home survive it.
    #[error("the repovec user home must be {expected}: {actual}")]
    RepovecHomeIncorrect {
        /// The home directory the contract requires.
        expected: &'static str,
        /// The home directory recorded in `/etc/passwd`.
        actual: String,
    },
    /// The `repovec` account exists but its login shell is not the contract
    /// nologin shell.
    #[error("the repovec user shell must be {expected}: {actual}")]
    RepovecShellIncorrect {
        /// The shell the contract requires.
        expected: &'static str,
        /// The shell recorded in `/etc/passwd`.
        actual: String,
    },
    /// A required directory is missing from the live tree.
    #[error("missing directory {path}")]
    MissingPath {
        /// The missing required directory.
        path: Utf8PathBuf,
    },
    /// A required directory or account database could not be inspected.
    #[error("cannot inspect {path}: {}", .source.kind())]
    UnreadablePath {
        /// A required directory or database that could not be inspected.
        path: Utf8PathBuf,
        /// The underlying filesystem error.
        #[source]
        source: io::Error,
    },
    /// A required path exists but is not a directory.
    #[error("{path} must be a directory")]
    NotADirectory {
        /// The path that is not a directory.
        path: Utf8PathBuf,
    },
    /// A directory has a mode that differs from its contract expectation.
    ///
    /// The comparison is exact rather than one-directional: a directory that is
    /// more tightly restricted than the contract is just as unusable to the
    /// daemon as one that is too permissive, so both are reported.
    #[error("{} must have mode {}: {}", .violation.path, .violation.expected, .violation.actual)]
    IncorrectMode {
        /// The directory and mode that were violated.
        violation: LiveLayoutViolation,
    },
    /// A directory is owned by a different user than its contract requires.
    #[error("{} must be owned by {}: {}", .violation.path, .violation.expected, .violation.actual)]
    IncorrectOwner {
        /// The directory and expectation that were violated.
        violation: LiveLayoutViolation,
    },
    /// A directory belongs to a different group than its contract requires.
    #[error("{} must be in group {}: {}", .violation.path, .violation.expected, .violation.actual)]
    IncorrectGroup {
        /// The directory and expectation that were violated.
        violation: LiveLayoutViolation,
    },
}

impl LiveLayoutError {
    /// The directory path at fault, for structured logging.
    #[must_use]
    pub fn path(&self) -> Option<&str> {
        match self {
            Self::RepovecUserNotFound
            | Self::RepovecGroupNotFound
            | Self::RepovecUserIsRoot { .. }
            | Self::RepovecHomeIncorrect { .. }
            | Self::RepovecShellIncorrect { .. } => None,
            Self::MissingPath { path }
            | Self::UnreadablePath { path, .. }
            | Self::NotADirectory { path }
            | Self::IncorrectMode { violation: LiveLayoutViolation { path, .. } }
            | Self::IncorrectOwner { violation: LiveLayoutViolation { path, .. } }
            | Self::IncorrectGroup { violation: LiveLayoutViolation { path, .. } } => {
                Some(path.as_str())
            }
        }
    }

    /// The stable, bounded category label for this failure.
    ///
    /// Metrics and failure logs carry the category, never the filesystem path,
    /// so a dashboard label stays bounded and no host layout leaks into it.
    #[must_use]
    pub const fn category(&self) -> &'static str {
        match self {
            Self::RepovecUserNotFound => "repovec_user_not_found",
            Self::RepovecGroupNotFound => "repovec_group_not_found",
            Self::RepovecUserIsRoot { .. } => "repovec_user_is_root",
            Self::RepovecHomeIncorrect { .. } => "repovec_home_incorrect",
            Self::RepovecShellIncorrect { .. } => "repovec_shell_incorrect",
            Self::MissingPath { .. } => "missing_path",
            Self::UnreadablePath { .. } => "unreadable_path",
            Self::NotADirectory { .. } => "not_a_directory",
            Self::IncorrectMode { .. } => "incorrect_mode",
            Self::IncorrectOwner { .. } => "incorrect_owner",
            Self::IncorrectGroup { .. } => "incorrect_group",
        }
    }

    /// The expected and observed values, bounded for diagnostic logging.
    ///
    /// Owner, group, and mode violations retain both sides of the comparison,
    /// but the error display names only the expectation. `MissingPath`,
    /// `NotADirectory`, and `RepovecUserNotFound` are self-describing. The
    /// account-database and security violations report the expected contract
    /// and the offending value. Every other variant has no observed value to
    /// report, so both fields are empty.
    #[must_use]
    pub fn diagnostic_values(&self) -> (&str, &str) {
        match self {
            Self::IncorrectMode { violation }
            | Self::IncorrectOwner { violation }
            | Self::IncorrectGroup { violation } => {
                (violation.expected.as_str(), violation.actual.as_str())
            }
            Self::RepovecUserIsRoot { .. } => ("a non-root uid", "root (uid 0)"),
            Self::RepovecHomeIncorrect { expected, actual }
            | Self::RepovecShellIncorrect { expected, actual } => (expected, actual.as_str()),
            Self::RepovecUserNotFound
            | Self::RepovecGroupNotFound
            | Self::MissingPath { .. }
            | Self::UnreadablePath { .. }
            | Self::NotADirectory { .. } => ("", ""),
        }
    }
}

/// Verifies the live layout against explicitly supplied account ids.
///
/// This is the testable seam for [`verify_live_layout`]: callers (including
/// tests running under an unprivileged account) provide the expected
/// `repovec` uid/gid directly.
///
/// # Examples
///
/// ```no_run
/// use repovec_core::RuntimePaths;
/// use repovec_core::appliance::directory_layout::live::{LiveLayoutIds, verify_layout_for_ids};
///
/// let ids = LiveLayoutIds {
///     repovec_uid: 998,
///     repovec_gid: 996,
///     privileged_uid: 0,
///     privileged_gid: 0,
/// };
///
/// verify_layout_for_ids(&RuntimePaths::appliance_defaults(), ids)
///     .expect("the host is provisioned");
/// ```
///
/// # Errors
///
/// Returns [`LiveLayoutError`] when a required directory is missing, not a
/// directory, unreadable, mis-moded, or misowned.
pub fn verify_layout_for_ids(
    paths: &RuntimePaths,
    ids: LiveLayoutIds,
) -> Result<(), LiveLayoutError> {
    layout_contracts(paths, ids)
        .into_iter()
        .try_for_each(|(path, contract)| verify_directory(&path, contract))
}

/// Pairs every required directory with its ownership and mode contract.
///
/// The mapping is the heart of the pre-flight: the data tree is owned by the
/// `repovec` account (SI-1), `qdrant-storage` by root (SI-2), and `/etc/repovec`
/// by root with the `repovec` group (SI-3). Returning it as data lets tests pin
/// the id-to-directory mapping without provisioning a root-owned tree, which an
/// unprivileged test process cannot create.
///
/// The five data-tree contracts come from the static validator spec table.
/// `/etc/repovec` is appended separately because the API-key helper, not
/// `tmpfiles.d`, owns that directory.
pub(crate) fn layout_contracts(
    paths: &RuntimePaths,
    ids: LiveLayoutIds,
) -> Vec<(Utf8PathBuf, DirectoryContract)> {
    let secrets_contract = DirectoryContract {
        owner_uid: ids.privileged_uid,
        owner_gid: ids.repovec_gid,
        owner_label: "root",
        group_label: "repovec",
        mode: 0o750,
    };

    let mut contracts: Vec<_> = super::layout_contract(paths)
        .into_iter()
        .map(|spec| {
            let contract = DirectoryContract {
                owner_uid: id_for(spec.owner, ids.repovec_uid, ids.privileged_uid),
                owner_gid: id_for(spec.group, ids.repovec_gid, ids.privileged_gid),
                owner_label: spec.owner,
                group_label: spec.group,
                mode: spec.mode.0,
            };
            (spec.path, contract)
        })
        .collect();
    contracts.push((paths.config_root().to_path_buf(), secrets_contract));
    contracts
}

/// Maps the two names used by `DirectorySpec` to their independently resolved ids.
fn id_for(name: &str, repovec: u32, privileged: u32) -> u32 {
    if name == super::REPOVEC_USER { repovec } else { privileged }
}
