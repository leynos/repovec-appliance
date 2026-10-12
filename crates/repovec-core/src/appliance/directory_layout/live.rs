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

use std::io;

use camino::Utf8PathBuf;
use cap_std::{ambient_authority, fs_utf8::Dir};

use super::{
    directory_contract::{DirectoryContract, verify_directory},
    group::{GROUP_DATABASE_PATH, GroupLookupError, lookup_repovec_gid},
};
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
    /// A required directory is missing from the live tree.
    #[error("missing directory {path}")]
    MissingPath {
        /// The missing required directory.
        path: Utf8PathBuf,
    },
    /// A required directory could not be inspected.
    #[error("cannot inspect {path}")]
    UnreadablePath {
        /// A required directory that could not be inspected.
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
    #[error("{} must be owned by {}", .violation.path, .violation.expected)]
    IncorrectOwner {
        /// The directory and expectation that were violated.
        violation: LiveLayoutViolation,
    },
    /// A directory belongs to a different group than its contract requires.
    #[error("{} must be in group {}", .violation.path, .violation.expected)]
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
            Self::RepovecUserNotFound | Self::RepovecGroupNotFound => None,
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
}

/// Expected account ids for a live layout pre-flight.
///
/// The production path resolves the `repovec` ids by name from `/etc/passwd`
/// and `/etc/group` and pins the storage and secrets owner to root (uid 0,
/// gid 0). The testable seam [`verify_layout_for_ids`] takes the ids explicitly
/// because a scratch tree owned by an unprivileged test process cannot be
/// granted to root.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LiveLayoutIds {
    /// The `repovec` account uid; owner of the private data tree (SI-1).
    pub repovec_uid: u32,
    /// The `repovec` account gid.
    pub repovec_gid: u32,
    /// The uid that must own `qdrant-storage` and `/etc/repovec` (root).
    pub privileged_uid: u32,
    /// The gid that must own `qdrant-storage` (root).
    pub privileged_gid: u32,
}

/// The root ids assumed for the privileged directories.
///
/// `qdrant-storage` and `/etc/repovec` are root-owned on a real host (SI-2,
/// SI-3). Tests inject different ids because a scratch tree cannot be chowned
/// to root without privileges.
const PRIVILEGED_UID: u32 = 0;
const PRIVILEGED_GID: u32 = 0;

/// Verifies the live on-disk layout for the standard appliance paths.
///
/// Resolves the `repovec` account ids from `/etc/passwd` and `/etc/group` and
/// requires every directory in the layout contract to exist with the documented
/// mode and ownership.
///
/// # Examples
///
/// ```no_run
/// use repovec_core::RuntimePaths;
/// use repovec_core::appliance::directory_layout::live::verify_live_layout;
///
/// verify_live_layout(&RuntimePaths::appliance_defaults()).expect("the host is provisioned");
/// ```
///
/// # Errors
///
/// Returns [`LiveLayoutError`] when the `repovec` account cannot be resolved
/// or any required directory is missing, not a directory, unreadable,
/// mis-moded, or misowned.
pub fn verify_live_layout(paths: &RuntimePaths) -> Result<(), LiveLayoutError> {
    verify_layout_for_ids(paths, ResolvedRepovec::resolve()?.into_ids())
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
    let data_contract = DirectoryContract {
        owner_uid: ids.repovec_uid,
        owner_gid: ids.repovec_gid,
        owner_label: "repovec",
        group_label: "repovec",
        mode: 0o700,
    };
    let storage_contract = DirectoryContract {
        owner_uid: ids.privileged_uid,
        owner_gid: ids.privileged_gid,
        owner_label: "root",
        group_label: "root",
        mode: 0o700,
    };
    let secrets_contract = DirectoryContract {
        owner_uid: ids.privileged_uid,
        owner_gid: ids.repovec_gid,
        owner_label: "root",
        group_label: "repovec",
        mode: 0o750,
    };

    verify_directory(paths.data_root(), data_contract)?;
    verify_directory(&paths.git_mirrors_root(), data_contract)?;
    verify_directory(&paths.worktrees_root(), data_contract)?;
    verify_directory(&paths.grepai_root(), data_contract)?;
    verify_directory(&paths.qdrant_storage_root(), storage_contract)?;
    verify_directory(paths.config_root(), secrets_contract)
}

/// The `repovec` uid and gid resolved from the account databases.
///
/// A named pair rather than a tuple keeps the uid and gid distinguishable at
/// every use site, and keeps them from colliding with the identically-shaped
/// [`LiveLayoutIds`] fields at the call site.
#[derive(Clone, Copy, Debug)]
struct ResolvedRepovec {
    uid: u32,
    gid: u32,
}

impl ResolvedRepovec {
    /// Reads the `repovec` uid from `/etc/passwd` and gid from `/etc/group`.
    ///
    /// This is deliberately in the live adapter: resolving ownership by name is
    /// a runtime concern (the pure validator never touches the live
    /// filesystem).
    fn resolve() -> Result<Self, LiveLayoutError> {
        let etc = Dir::open_ambient_dir("/etc", ambient_authority()).map_err(|source| {
            LiveLayoutError::UnreadablePath { path: Utf8PathBuf::from("/etc/passwd"), source }
        })?;
        let passwd = etc.read_to_string("passwd").map_err(|source| {
            LiveLayoutError::UnreadablePath { path: Utf8PathBuf::from("/etc/passwd"), source }
        })?;

        let uid = find_repovec_uid(&passwd).ok_or(LiveLayoutError::RepovecUserNotFound)?;
        let gid = lookup_repovec_gid().map_err(|error| match error {
            GroupLookupError::Unreadable(source) => LiveLayoutError::UnreadablePath {
                path: Utf8PathBuf::from(GROUP_DATABASE_PATH),
                source,
            },
            GroupLookupError::NotFound => LiveLayoutError::RepovecGroupNotFound,
        })?;

        Ok(Self { uid, gid })
    }

    /// Pairs the resolved account ids with the privileged root ids.
    const fn into_ids(self) -> LiveLayoutIds {
        LiveLayoutIds {
            repovec_uid: self.uid,
            repovec_gid: self.gid,
            privileged_uid: PRIVILEGED_UID,
            privileged_gid: PRIVILEGED_GID,
        }
    }
}

/// Extracts the `repovec` uid from passwd text.
///
/// Only the uid is taken from `passwd`; the group is resolved separately from
/// the group database (see [`ResolvedRepovec::resolve`]).
pub(crate) fn find_repovec_uid(passwd: &str) -> Option<u32> {
    passwd.lines().find_map(|line| {
        let mut fields = line.split(':');
        if fields.next()? != "repovec" {
            return None;
        }
        let _password = fields.next()?;
        fields.next()?.parse().ok()
    })
}
