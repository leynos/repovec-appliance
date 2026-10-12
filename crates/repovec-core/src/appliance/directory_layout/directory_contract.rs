//! Ownership, group, and mode checks for one live layout directory.
//!
//! This module holds the per-directory half of the live pre-flight. The parent
//! [`super::live`] module decides *which* directories the contract covers and in
//! what order; this module answers, for a single path, whether the metadata on
//! disk matches the expected owner, group, and mode.
//!
//! The split keeps each file small enough to review as a unit and puts the
//! error-mapping rules (missing versus unreadable, directory versus non-
//! directory) beside the syscalls that can produce them.

use std::io;

use camino::Utf8Path;
use cap_std::{
    ambient_authority,
    fs::MetadataExt,
    fs_utf8::{Dir, Metadata},
};

use super::live::{LiveLayoutError, LiveLayoutViolation};

/// The ownership and permission contract for one directory in the layout.
#[derive(Clone, Copy, Debug)]
pub(super) struct DirectoryContract {
    /// The uid that must own the directory.
    pub(super) owner_uid: u32,
    /// The gid the directory must belong to.
    pub(super) owner_gid: u32,
    /// The owner name rendered in diagnostics.
    pub(super) owner_label: &'static str,
    /// The group name rendered in diagnostics.
    pub(super) group_label: &'static str,
    /// The exact octal mode the directory must carry.
    pub(super) mode: u16,
}

impl DirectoryContract {
    /// The operator-facing description of the required owner.
    fn expected_owner(self) -> String { format!("{} (uid {})", self.owner_label, self.owner_uid) }

    /// The operator-facing description of the required group.
    fn expected_group(self) -> String { format!("{} (gid {})", self.group_label, self.owner_gid) }

    /// The operator-facing description of the required mode.
    fn expected_mode(self) -> String { format!("{:04o}", self.mode) }
}

/// Verifies one directory against its ownership and permission contract.
pub(super) fn verify_directory(
    path: &Utf8Path,
    contract: DirectoryContract,
) -> Result<(), LiveLayoutError> {
    let metadata = read_directory_metadata(path)?;
    check_owner(path, &metadata, contract)?;
    check_group(path, &metadata, contract)?;
    check_mode(path, &metadata, contract)
}

/// Rejects a directory owned by an account other than the expected one.
fn check_owner(
    path: &Utf8Path,
    metadata: &Metadata,
    contract: DirectoryContract,
) -> Result<(), LiveLayoutError> {
    if metadata.uid() == contract.owner_uid {
        return Ok(());
    }
    Err(LiveLayoutError::IncorrectOwner {
        violation: LiveLayoutViolation {
            path: path.to_path_buf(),
            expected: contract.expected_owner(),
            actual: format!("uid {}", metadata.uid()),
        },
    })
}

fn check_group(
    path: &Utf8Path,
    metadata: &Metadata,
    contract: DirectoryContract,
) -> Result<(), LiveLayoutError> {
    if metadata.gid() == contract.owner_gid {
        return Ok(());
    }
    Err(LiveLayoutError::IncorrectGroup {
        violation: LiveLayoutViolation {
            path: path.to_path_buf(),
            expected: contract.expected_group(),
            actual: format!("gid {}", metadata.gid()),
        },
    })
}

fn check_mode(
    path: &Utf8Path,
    metadata: &Metadata,
    contract: DirectoryContract,
) -> Result<(), LiveLayoutError> {
    let actual_mode = metadata.mode() & 0o777;
    if actual_mode == u32::from(contract.mode) {
        return Ok(());
    }
    Err(LiveLayoutError::IncorrectMode {
        violation: LiveLayoutViolation {
            path: path.to_path_buf(),
            expected: contract.expected_mode(),
            actual: format!("{actual_mode:04o}"),
        },
    })
}

/// Reads the metadata of a required layout directory.
///
/// The metadata must describe a directory: a regular file left at the same path
/// with the expected owner, group, and mode would otherwise satisfy every
/// subsequent check while leaving the documented directory unusable.
fn read_directory_metadata(path: &Utf8Path) -> Result<Metadata, LiveLayoutError> {
    let (Some(parent), Some(name)) = (path.parent(), path.file_name()) else {
        return Err(LiveLayoutError::UnreadablePath {
            path: path.to_path_buf(),
            source: io::Error::new(io::ErrorKind::InvalidInput, "path must name a directory"),
        });
    };

    let directory = Dir::open_ambient_dir(parent, ambient_authority())
        .map_err(|source| map_metadata_error(path, source))?;
    let metadata = directory.metadata(name).map_err(|source| map_metadata_error(path, source))?;

    if metadata.is_dir() {
        Ok(metadata)
    } else {
        Err(LiveLayoutError::NotADirectory { path: path.to_path_buf() })
    }
}

/// Maps a filesystem error to the typed error, distinguishing a missing
/// required directory from a directory that exists but cannot be inspected.
fn map_metadata_error(path: &Utf8Path, source: io::Error) -> LiveLayoutError {
    if source.kind() == io::ErrorKind::NotFound {
        LiveLayoutError::MissingPath { path: path.to_path_buf() }
    } else {
        LiveLayoutError::UnreadablePath { path: path.to_path_buf(), source }
    }
}
