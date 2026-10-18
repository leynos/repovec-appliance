//! Account resolution for the live directory-layout pre-flight.
//!
//! The pre-flight needs the `repovec` uid and gid before it can stat a single
//! directory. This module reads `/etc/passwd` and `/etc/group` once, validates
//! the whole `passwd` record, and resolves the group from the group database
//! rather than from the user's primary GID. The directory checks themselves
//! live in [`super::live`].
//!
//! Splitting the read-and-resolve half out of the verification half keeps both
//! files reviewable and lets tests drive every resolution rule from captured
//! database text instead of the host's real accounts.

use std::io;

use camino::Utf8PathBuf;

use super::{
    account::{
        PASSWD_DATABASE_PATH, find_repovec_account, passwd_database_text, validate_repovec_account,
    },
    group::{GROUP_DATABASE_PATH, GroupLookupError, group_database_text, resolve_repovec_gid_from},
    live::{LiveLayoutError, verify_layout_for_ids},
};
use crate::RuntimePaths;

/// Expected account ids for a live layout pre-flight.
///
/// The production path resolves the `repovec` ids by name from `/etc/passwd`
/// and `/etc/group` and requires the storage and secrets directories to be
/// root-owned. The testable seam [`super::live::verify_layout_for_ids`] takes
/// the ids explicitly because a scratch tree owned by an unprivileged test
/// process cannot be granted to root.
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
/// Reads `/etc/passwd` and `/etc/group` once, resolves the `repovec` ids by
/// name, and requires every directory in the layout contract to exist with the
/// documented mode and ownership.
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
/// Returns [`LiveLayoutError`] when the account databases cannot be read, the
/// `repovec` account or group is absent or unsafe, or any required directory is
/// missing, not a directory, unreadable, mis-moded, or misowned.
pub fn verify_live_layout(paths: &RuntimePaths) -> Result<(), LiveLayoutError> {
    let passwd = passwd_database_text()
        .map_err(|source| unreadable_database(PASSWD_DATABASE_PATH, source))?;
    let group = group_database_text().map_err(map_group_lookup_error)?;

    verify_layout_from_databases(paths, &passwd, &group)
}

/// Verifies the live layout against captured account-database text.
///
/// This is the seam [`verify_live_layout`] uses after reading `/etc/passwd` and
/// `/etc/group`, and it is what tests drive so every resolution rule is
/// exercised without depending on the host's real accounts.
///
/// # Errors
///
/// Returns [`LiveLayoutError`] when the records are missing, truncated, or
/// unsafe, or any required directory is missing, not a directory, unreadable,
/// mis-moded, or misowned.
pub(crate) fn verify_layout_from_databases(
    paths: &RuntimePaths,
    passwd: &str,
    group: &str,
) -> Result<(), LiveLayoutError> {
    verify_layout_for_ids(paths, resolve_ids_from_databases(passwd, group)?)
}

/// Resolves the expected ids from captured account-database text.
///
/// The group is resolved from the group database, never from the user's
/// primary gid: `systemd-tmpfiles` resolves its group column by name, so a
/// correctly provisioned host can carry a `repovec` user whose primary group is
/// something other than the `repovec` group.
///
/// # Errors
///
/// Returns [`LiveLayoutError::RepovecUserNotFound`] when the account row is
/// absent or truncated, the account-validation errors when the row is unsafe,
/// and [`LiveLayoutError::RepovecGroupNotFound`] when the group row is absent.
pub(crate) fn resolve_ids_from_databases(
    passwd: &str,
    group: &str,
) -> Result<LiveLayoutIds, LiveLayoutError> {
    let account = find_repovec_account(passwd).ok_or(LiveLayoutError::RepovecUserNotFound)?;
    validate_repovec_account(&account)?;
    let gid = resolve_repovec_gid_from(group).map_err(map_group_lookup_error)?;

    Ok(LiveLayoutIds {
        repovec_uid: account.uid,
        repovec_gid: gid,
        // The privileged directories are root-owned on a real host (SI-2,
        // SI-3). They are fixed here rather than taken from the account
        // databases: no host account should ever be able to own them.
        privileged_uid: 0,
        privileged_gid: 0,
    })
}

/// Maps a group-database failure to the live-layout error.
///
/// `group_database_text` only reports unreadable input; an absent row belongs
/// to the resolution step, which reports it as a missing group.
fn map_group_lookup_error(error: GroupLookupError) -> LiveLayoutError {
    match error {
        GroupLookupError::Unreadable(source) => unreadable_database(GROUP_DATABASE_PATH, source),
        GroupLookupError::NotFound => LiveLayoutError::RepovecGroupNotFound,
    }
}

/// Maps an unreadable account database to the typed error.
fn unreadable_database(path: &str, source: io::Error) -> LiveLayoutError {
    LiveLayoutError::UnreadablePath { path: Utf8PathBuf::from(path), source }
}
