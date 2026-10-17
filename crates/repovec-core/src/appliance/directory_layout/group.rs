//! Group-database lookup for the live directory-layout pre-flight.
//!
//! `systemd-tmpfiles` resolves the `repovec` group column of the layout asset
//! through the group database, not through the `repovec` user's `passwd` entry.
//! A host can therefore carry a `repovec` user whose primary group is something
//! else (for example an inherited packaging account) while a separate `repovec`
//! group owns the data tree. Deriving the expected group from the user's
//! primary GID would reject those correctly provisioned directories, so the
//! live adapter reads `/etc/group` instead.
//!
//! [`lookup_repovec_gid`] is the live file-reading adapter; [`find_group_gid`]
//! is the pure parse seam the unit tests drive directly.

use std::io;

use cap_std::{ambient_authority, fs_utf8::Dir};

/// The installed path of the group database consulted by the live pre-flight.
pub(crate) const GROUP_DATABASE_PATH: &str = "/etc/group";

/// Why the `repovec` group could not be resolved.
#[derive(Debug)]
pub(crate) enum GroupLookupError {
    /// The group database could not be read.
    Unreadable(io::Error),
    /// The group database was read, but declares no `repovec` group.
    NotFound,
}

impl std::fmt::Display for GroupLookupError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Unreadable(source) => {
                write!(formatter, "cannot read {GROUP_DATABASE_PATH}: {source}")
            }
            Self::NotFound => write!(formatter, "the repovec system group does not exist"),
        }
    }
}

impl std::error::Error for GroupLookupError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Unreadable(source) => Some(source),
            Self::NotFound => None,
        }
    }
}

/// Resolves the `gid` of the `repovec` group from the live group database.
///
/// # Errors
///
/// Returns [`GroupLookupError::Unreadable`] when `/etc/group` cannot be opened
/// or read, and [`GroupLookupError::NotFound`] when it declares no `repovec`
/// group.
pub(crate) fn lookup_repovec_gid() -> Result<u32, GroupLookupError> { lookup_gid("repovec") }

/// Resolves the `gid` of `name` from the live group database.
fn lookup_gid(name: &str) -> Result<u32, GroupLookupError> {
    let etc =
        Dir::open_ambient_dir("/etc", ambient_authority()).map_err(GroupLookupError::Unreadable)?;
    let contents = etc.read_to_string("group").map_err(GroupLookupError::Unreadable)?;

    find_group_gid(&contents, name).ok_or(GroupLookupError::NotFound)
}

/// Extracts the `gid` of `name` from group-database text.
///
/// The database is parsed with the same strictness as the checked-in assets:
/// a row contributes only when it carries the full `name:password:gid:members`
/// shape and its `gid` column parses as a number. Rows for other groups are
/// skipped, so the lookup does not depend on the row order.
pub(crate) fn find_group_gid(contents: &str, name: &str) -> Option<u32> {
    contents.lines().find_map(|line| {
        let mut columns = line.split(':');
        if columns.next()? != name {
            return None;
        }
        let _password = columns.next()?;
        let gid = columns.next()?.parse().ok()?;
        // Every group row carries the members column, even when it is empty.
        columns.next()?;
        Some(gid)
    })
}
