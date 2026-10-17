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

/// One field of the per-directory contract, expressed as data.
///
/// Owner, group, and mode are checked by the same three-step decision—does the
/// metadata satisfy the contract, what did the contract require, what is
/// actually there—and differ only in the predicate, the two message fragments,
/// and the error variant. Holding those four pieces together lets the checks run
/// as one ordered pass instead of three near-identical functions.
struct Expectation {
    /// Whether the directory metadata satisfies this field of the contract.
    is_satisfied: fn(&Metadata, DirectoryContract) -> bool,
    /// The operator-facing description of the required value.
    expected: fn(DirectoryContract) -> String,
    /// The operator-facing description of the value found on disk.
    reported: fn(&Metadata) -> String,
    /// The error variant this field reports when it is unsatisfied.
    violation: fn(LiveLayoutViolation) -> LiveLayoutError,
}

impl Expectation {
    /// Returns the violation for `path` when the metadata fails this field.
    fn check(
        self,
        path: &Utf8Path,
        metadata: &Metadata,
        contract: DirectoryContract,
    ) -> Result<(), LiveLayoutError> {
        if (self.is_satisfied)(metadata, contract) {
            return Ok(());
        }

        Err((self.violation)(LiveLayoutViolation {
            path: path.to_path_buf(),
            expected: (self.expected)(contract),
            actual: (self.reported)(metadata),
        }))
    }
}

/// The contract fields, in the order their violations are reported.
///
/// Order matters: `verify_directory` returns the first unsatisfied field, so the
/// list runs from the most fundamental mismatch (owner) to the least (mode).
const EXPECTATIONS: [Expectation; 3] = [
    Expectation {
        is_satisfied: owner_is_satisfied,
        expected: DirectoryContract::expected_owner,
        reported: reported_owner,
        violation: |violation| LiveLayoutError::IncorrectOwner { violation },
    },
    Expectation {
        is_satisfied: group_is_satisfied,
        expected: DirectoryContract::expected_group,
        reported: reported_group,
        violation: |violation| LiveLayoutError::IncorrectGroup { violation },
    },
    Expectation {
        is_satisfied: mode_is_satisfied,
        expected: DirectoryContract::expected_mode,
        reported: reported_mode,
        violation: |violation| LiveLayoutError::IncorrectMode { violation },
    },
];

/// The permission bits of the metadata, with the file-type bits masked off.
///
/// The mask keeps the set-user-ID, set-group-ID, and sticky bits as well as the
/// nine ordinary permission bits. Masking with `0o777` would make `02700` and
/// `01700` compare equal to the contracted `0700`, and would hide the special
/// bits from the diagnostic, so the live pre-flight would accept a directory
/// that does not match the exact-mode contract the pure validator enforces.
fn permission_bits(metadata: &Metadata) -> u32 { metadata.mode() & 0o7777 }

fn owner_is_satisfied(metadata: &Metadata, contract: DirectoryContract) -> bool {
    metadata.uid() == contract.owner_uid
}

fn group_is_satisfied(metadata: &Metadata, contract: DirectoryContract) -> bool {
    metadata.gid() == contract.owner_gid
}

fn mode_is_satisfied(metadata: &Metadata, contract: DirectoryContract) -> bool {
    permission_bits(metadata) == u32::from(contract.mode)
}

fn reported_owner(metadata: &Metadata) -> String { format!("uid {}", metadata.uid()) }

fn reported_group(metadata: &Metadata) -> String { format!("gid {}", metadata.gid()) }

fn reported_mode(metadata: &Metadata) -> String { format!("{:04o}", permission_bits(metadata)) }

/// Verifies one directory against its ownership and permission contract.
pub(super) fn verify_directory(
    path: &Utf8Path,
    contract: DirectoryContract,
) -> Result<(), LiveLayoutError> {
    let metadata = read_directory_metadata(path)?;
    EXPECTATIONS
        .into_iter()
        .try_for_each(|expectation| expectation.check(path, &metadata, contract))
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
