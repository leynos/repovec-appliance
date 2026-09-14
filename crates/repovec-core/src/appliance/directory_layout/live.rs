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
//!   `.grepai/` children) must be owned by the `repovec` user and group with
//!   no group or other access (`mode & 0o077 == 0`, SI-1);
//! - `qdrant-storage` must be owned `root:root` with `mode & 0o077 == 0`
//!   (SI-2);
//! - `/etc/repovec` must be owned `root`, group `repovec`, non-world and
//!   non-group-write (`mode & 0o027 == 0`, SI-3).

#![allow(
    clippy::similar_names,
    reason = "the uid/gid pair field names are inherently similar by contract"
)]

use std::io;

use camino::Utf8PathBuf;
use cap_std::{ambient_authority, fs::MetadataExt, fs_utf8::Dir};

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
#[derive(Debug)]
pub enum LiveLayoutError {
    /// The `repovec` account is not present in `/etc/passwd`, so ownership
    /// cannot be resolved by name.
    RepovecUserNotFound,
    /// A required directory is missing from the live tree.
    MissingPath {
        /// The missing required directory.
        path: Utf8PathBuf,
    },
    /// A required directory could not be inspected.
    UnreadablePath {
        /// A required directory that could not be inspected.
        path: Utf8PathBuf,
        /// The underlying filesystem error.
        source: io::Error,
    },
    /// A directory exposes more access than its contract allows.
    InsecureMode {
        /// The directory and expectation that were violated.
        violation: LiveLayoutViolation,
    },
    /// A directory is owned by a different user than its contract requires.
    IncorrectOwner {
        /// The directory and expectation that were violated.
        violation: LiveLayoutViolation,
    },
    /// A directory belongs to a different group than its contract requires.
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
            Self::RepovecUserNotFound => None,
            Self::MissingPath { path } | Self::UnreadablePath { path, .. } => Some(path.as_str()),
            Self::InsecureMode { violation }
            | Self::IncorrectOwner { violation }
            | Self::IncorrectGroup { violation } => Some(violation.path.as_str()),
        }
    }
}

impl std::fmt::Display for LiveLayoutError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::RepovecUserNotFound => {
                write!(f, "the repovec system user does not exist")
            }
            Self::MissingPath { path } => write!(f, "missing directory {path}"),
            Self::UnreadablePath { path, source } => {
                write!(f, "cannot inspect {path}: {source}")
            }
            Self::InsecureMode { violation } => {
                write!(f, "{} must not exceed permissions {}", violation.path, violation.expected)
            }
            Self::IncorrectOwner { violation } => {
                write!(f, "{} must be owned by {}", violation.path, violation.expected)
            }
            Self::IncorrectGroup { violation } => {
                write!(f, "{} must be in group {}", violation.path, violation.expected)
            }
        }
    }
}

impl std::error::Error for LiveLayoutError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::UnreadablePath { source, .. } => Some(source),
            _ => None,
        }
    }
}

/// Expected account ids for a live layout pre-flight.
///
/// The production path resolves the `repovec` ids by name from `/etc/passwd`
/// and pins the storage and secrets owner to root (uid 0, gid 0). The testable
/// seam [`verify_layout_for_ids`] takes the ids explicitly because a scratch
/// tree owned by an unprivileged test process cannot be granted to root.
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

/// Verifies the live on-disk layout for the standard appliance paths.
///
/// Resolves the `repovec` account ids from `/etc/passwd` and requires every
/// directory in the layout contract to exist with the documented mode and
/// ownership.
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
/// or any required directory is missing, unreadable, over-permissive, or
/// misowned.
pub fn verify_live_layout(paths: &RuntimePaths) -> Result<(), LiveLayoutError> {
    let (expected_owner_uid, expected_owner_gid) = resolve_repovec_ids()?;
    let ids = LiveLayoutIds {
        repovec_uid: expected_owner_uid,
        repovec_gid: expected_owner_gid,
        // The storage and secrets directories are root-owned (SI-2, SI-3);
        // uid/gid 0 is the production expectation. Tests inject explicit ids
        // because a temp tree cannot be chowned to root without privileges.
        privileged_uid: 0,
        privileged_gid: 0,
    };
    verify_layout_for_ids(paths, ids)
}

/// Verifies the live layout against explicitly supplied account ids.
///
/// This is the testable seam for [`verify_live_layout`]: callers (including
/// tests running under an unprivileged account) provide the expected
/// `repovec` uid/gid directly.
///
/// # Errors
///
/// Returns [`LiveLayoutError`] when a required directory is missing,
/// unreadable, over-permissive, or misowned.
pub fn verify_layout_for_ids(
    paths: &RuntimePaths,
    ids: LiveLayoutIds,
) -> Result<(), LiveLayoutError> {
    let data_root = paths.data_root();
    verify_data_directory(data_root, ids.repovec_uid, ids.repovec_gid)?;
    verify_data_directory(&paths.git_mirrors_root(), ids.repovec_uid, ids.repovec_gid)?;
    verify_data_directory(&paths.worktrees_root(), ids.repovec_uid, ids.repovec_gid)?;
    verify_data_directory(&paths.grepai_root(), ids.repovec_uid, ids.repovec_gid)?;

    let storage = paths.qdrant_storage_root();
    verify_root_private_directory(&storage, ids.privileged_uid, ids.privileged_gid)?;

    let config_root = paths.config_root();
    verify_secrets_directory(config_root, ids.privileged_uid, ids.repovec_gid)?;

    Ok(())
}

/// Resolves the `repovec` account uid/gid from `/etc/passwd`.
///
/// This is deliberately in the live adapter: resolving ownership by name is a
/// runtime concern (the pure validator never touches the live filesystem).
fn resolve_repovec_ids() -> Result<(u32, u32), LiveLayoutError> {
    let etc = Dir::open_ambient_dir("/etc", ambient_authority()).map_err(|source| {
        LiveLayoutError::UnreadablePath { path: Utf8PathBuf::from("/etc/passwd"), source }
    })?;
    let passwd = etc.read_to_string("passwd").map_err(|source| {
        LiveLayoutError::UnreadablePath { path: Utf8PathBuf::from("/etc/passwd"), source }
    })?;

    find_repovec_ids(&passwd).ok_or(LiveLayoutError::RepovecUserNotFound)
}

/// Extracts the `repovec` uid/gid from passwd text.
pub(crate) fn find_repovec_ids(passwd: &str) -> Option<(u32, u32)> {
    passwd.lines().find_map(|line| {
        let mut fields = line.split(':');
        if fields.next()? != "repovec" {
            return None;
        }
        let _password = fields.next()?;
        let uid = fields.next()?.parse().ok()?;
        let gid = fields.next()?.parse().ok()?;
        Some((uid, gid))
    })
}

fn verify_data_directory(
    path: &camino::Utf8Path,
    expected_owner_uid: u32,
    expected_owner_gid: u32,
) -> Result<(), LiveLayoutError> {
    let metadata = read_directory_metadata(path)?;

    if metadata.uid() != expected_owner_uid {
        return Err(LiveLayoutError::IncorrectOwner {
            violation: LiveLayoutViolation {
                path: path.to_path_buf(),
                expected: format!("uid {expected_owner_uid} (repovec)"),
                actual: format!("uid {}", metadata.uid()),
            },
        });
    }
    if metadata.gid() != expected_owner_gid {
        return Err(LiveLayoutError::IncorrectGroup {
            violation: LiveLayoutViolation {
                path: path.to_path_buf(),
                expected: format!("gid {expected_owner_gid} (repovec)"),
                actual: format!("gid {}", metadata.gid()),
            },
        });
    }
    if metadata.mode() & 0o077 != 0 {
        return Err(LiveLayoutError::InsecureMode {
            violation: LiveLayoutViolation {
                path: path.to_path_buf(),
                expected: String::from("no group/other access (0700)"),
                actual: format!("{:04o}", metadata.mode() & 0o777),
            },
        });
    }
    Ok(())
}

fn verify_root_private_directory(
    path: &camino::Utf8Path,
    expected_uid: u32,
    expected_gid: u32,
) -> Result<(), LiveLayoutError> {
    let metadata = read_directory_metadata(path)?;

    if metadata.uid() != expected_uid {
        return Err(LiveLayoutError::IncorrectOwner {
            violation: LiveLayoutViolation {
                path: path.to_path_buf(),
                expected: format!("uid {expected_uid} (root)"),
                actual: format!("uid {}", metadata.uid()),
            },
        });
    }
    if metadata.gid() != expected_gid {
        return Err(LiveLayoutError::IncorrectGroup {
            violation: LiveLayoutViolation {
                path: path.to_path_buf(),
                expected: format!("gid {expected_gid} (root)"),
                actual: format!("gid {}", metadata.gid()),
            },
        });
    }
    if metadata.mode() & 0o077 != 0 {
        return Err(LiveLayoutError::InsecureMode {
            violation: LiveLayoutViolation {
                path: path.to_path_buf(),
                expected: String::from("no group/other access (0700)"),
                actual: format!("{:04o}", metadata.mode() & 0o777),
            },
        });
    }
    Ok(())
}

fn verify_secrets_directory(
    path: &camino::Utf8Path,
    secrets_uid: u32,
    expected_owner_gid: u32,
) -> Result<(), LiveLayoutError> {
    let metadata = read_directory_metadata(path)?;

    if metadata.uid() != secrets_uid {
        return Err(LiveLayoutError::IncorrectOwner {
            violation: LiveLayoutViolation {
                path: path.to_path_buf(),
                expected: format!("uid {secrets_uid} (secrets owner)"),
                actual: format!("uid {}", metadata.uid()),
            },
        });
    }
    if metadata.gid() != expected_owner_gid {
        return Err(LiveLayoutError::IncorrectGroup {
            violation: LiveLayoutViolation {
                path: path.to_path_buf(),
                expected: format!("gid {expected_owner_gid} (repovec)"),
                actual: format!("gid {}", metadata.gid()),
            },
        });
    }
    // root-owned, group repovec, non-world and non-group-write (0750).
    if metadata.mode() & 0o027 != 0 {
        return Err(LiveLayoutError::InsecureMode {
            violation: LiveLayoutViolation {
                path: path.to_path_buf(),
                expected: String::from("root-owned, group repovec, non-world (0750)"),
                actual: format!("{:04o}", metadata.mode() & 0o777),
            },
        });
    }
    Ok(())
}

fn read_directory_metadata(
    path: &camino::Utf8Path,
) -> Result<cap_std::fs_utf8::Metadata, LiveLayoutError> {
    let Some(parent) = path.parent() else {
        return Err(LiveLayoutError::UnreadablePath {
            path: path.to_path_buf(),
            source: io::Error::new(io::ErrorKind::InvalidInput, "path must name a directory"),
        });
    };
    let Some(name) = path.file_name() else {
        return Err(LiveLayoutError::UnreadablePath {
            path: path.to_path_buf(),
            source: io::Error::new(io::ErrorKind::InvalidInput, "path must name a directory"),
        });
    };

    let directory = Dir::open_ambient_dir(parent, ambient_authority())
        .map_err(|source| map_metadata_error(path, source))?;

    directory.metadata(name).map_err(|source| map_metadata_error(path, source))
}

/// Maps a filesystem error to the typed error, distinguishing a missing
/// required directory from a directory that exists but cannot be inspected.
fn map_metadata_error(path: &camino::Utf8Path, source: io::Error) -> LiveLayoutError {
    if source.kind() == io::ErrorKind::NotFound {
        LiveLayoutError::MissingPath { path: path.to_path_buf() }
    } else {
        LiveLayoutError::UnreadablePath { path: path.to_path_buf(), source }
    }
}
