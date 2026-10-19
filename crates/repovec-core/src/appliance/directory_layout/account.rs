//! `passwd`-record parsing and validation for the live layout pre-flight.
//!
//! `systemd-sysusers` is idempotent in the strongest sense: when the account
//! already exists it reports it as provisioned and leaves every column alone.
//! A host that already carried a `repovec` entry therefore keeps its uid, home,
//! and shell no matter what the checked-in `sysusers.d` asset declares, and
//! `systemd-tmpfiles` will then hand the private data tree to whatever account
//! that row names. The live pre-flight is the only place the appliance can
//! refuse such a host before the daemons start, so it parses the whole record
//! rather than only the uid.
//!
//! [`find_repovec_account`] is the pure parse seam the unit tests drive; the
//! live file read lives in the parent module, next to the group lookup it is
//! paired with.

use std::io;

use cap_std::{ambient_authority, fs_utf8::Dir};

use super::{REPOVEC_HOME, REPOVEC_SHELL, REPOVEC_USER, live::LiveLayoutError};

/// The installed path of the account database consulted by the live pre-flight.
pub(crate) const PASSWD_DATABASE_PATH: &str = "/etc/passwd";

/// The root uid a `repovec` account must never resolve to.
const PRIVILEGED_UID: u32 = 0;

/// Reads the live account database so the resolution rules can run against a
/// captured snapshot.
///
/// Keeping the read separate from the parse lets the live pre-flight read each
/// database once, and lets tests drive every resolution rule—including
/// unreadable input—without touching `/etc`.
///
/// # Errors
///
/// Returns the underlying [`io::Error`] when `/etc/passwd` cannot be opened or
/// read.
pub(crate) fn passwd_database_text() -> io::Result<String> {
    let etc = Dir::open_ambient_dir("/etc", ambient_authority())?;

    etc.read_to_string("passwd")
}

/// The `repovec` account record read from `/etc/passwd`.
///
/// `systemd-sysusers` is idempotent: it reports an existing account as already
/// provisioned and leaves every column untouched. A host that already carried
/// a `repovec` entry therefore keeps its uid, home, and shell no matter what
/// the checked-in asset declares. Parsing the whole record lets the live
/// pre-flight reject such a host instead of letting `systemd-tmpfiles` hand the
/// data tree to an unintended account.
pub(crate) struct RepovecAccount {
    /// The account uid; the `systemd-tmpfiles` owner of the data tree.
    pub uid: u32,
    /// The account home directory.
    pub home: String,
    /// The account login shell.
    pub shell: String,
}

/// Extracts the `repovec` account record from passwd text.
///
/// The record is parsed with the strictness the asset validator applies to the
/// checked-in `sysusers.d` line: a row contributes only when it carries the
/// full `name:password:uid:gid:gecos:home:shell` shape and its uid column
/// parses as a number. Rows for other accounts are skipped, so the lookup does
/// not depend on row order. The group is resolved from the group database
/// separately.
///
/// A truncated row yields `None`: without the home and shell columns it cannot
/// prove the account satisfies the contract, and treating it as provisioned
/// would defeat the check.
pub(crate) fn find_repovec_account(passwd: &str) -> Option<RepovecAccount> {
    passwd.lines().find_map(|line| {
        let mut columns = line.split(':');
        if columns.next()? != REPOVEC_USER {
            return None;
        }
        let _password = columns.next()?;
        let uid = columns.next()?.parse().ok()?;
        let _gid = columns.next()?;
        let _gecos = columns.next()?;
        let home = columns.next()?;
        let shell = columns.next()?;
        Some(RepovecAccount { uid, home: home.to_owned(), shell: shell.to_owned() })
    })
}

/// Rejects a pre-existing `repovec` account that would subvert the contract.
///
/// `systemd-sysusers` cannot repair an account it did not create, so these
/// checks are the only place the appliance can refuse a root-owned or
/// login-capable `repovec` account before the daemons start.
///
/// # Errors
///
/// Returns [`LiveLayoutError::RepovecUserIsRoot`] when the uid is root,
/// [`LiveLayoutError::RepovecHomeIncorrect`] when the home is not
/// [`REPOVEC_HOME`], and [`LiveLayoutError::RepovecShellIncorrect`] when the
/// shell is not [`REPOVEC_SHELL`].
pub(crate) fn validate_repovec_account(account: &RepovecAccount) -> Result<(), LiveLayoutError> {
    if account.uid == PRIVILEGED_UID {
        return Err(LiveLayoutError::RepovecUserIsRoot { uid: account.uid });
    }
    if account.home != REPOVEC_HOME {
        return Err(LiveLayoutError::RepovecHomeIncorrect {
            expected: REPOVEC_HOME,
            actual: account.home.clone(),
        });
    }
    if account.shell != REPOVEC_SHELL {
        return Err(LiveLayoutError::RepovecShellIncorrect {
            expected: REPOVEC_SHELL,
            actual: account.shell.clone(),
        });
    }
    Ok(())
}
