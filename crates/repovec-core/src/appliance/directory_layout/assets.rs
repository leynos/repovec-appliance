//! Embedded `tmpfiles.d` and `sysusers.d` assets and their canonical paths.
//!
//! The two packaging assets that describe the `repovec` system user's
//! on-disk tree are embedded at compile time with [`include_str!`], so the
//! validators never read them from the filesystem at runtime. The constants
//! that name the assets, their repository paths, their installed paths, and
//! the account they create live here beside the assets they describe, keeping
//! the contract vocabulary in one reviewable place rather than mixed into the
//! validator entry points.

/// The embedded `tmpfiles.d` asset validated by the checked-in entry point.
pub(super) const CHECKED_IN_REPOVEC_TMPFILES: &str =
    include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/../../packaging/tmpfiles.d/repovec.conf"));
/// The embedded `sysusers.d` asset validated by the checked-in entry point.
pub(super) const CHECKED_IN_REPOVEC_SYSUSERS: &str =
    include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/../../packaging/sysusers.d/repovec.conf"));

/// The repository path of the checked-in `tmpfiles.d` asset.
pub const CHECKED_IN_REPOVEC_TMPFILES_PATH: &str = "packaging/tmpfiles.d/repovec.conf";
/// The repository path of the checked-in `sysusers.d` asset.
pub const CHECKED_IN_REPOVEC_SYSUSERS_PATH: &str = "packaging/sysusers.d/repovec.conf";
/// The installed path of the `tmpfiles.d` asset on a systemd host.
pub const INSTALLED_REPOVEC_TMPFILES_PATH: &str = "/usr/lib/tmpfiles.d/repovec.conf";
/// The installed path of the `sysusers.d` asset on a systemd host.
pub const INSTALLED_REPOVEC_SYSUSERS_PATH: &str = "/usr/lib/sysusers.d/repovec.conf";

/// The canonical `repovec` system user and group names.
pub const REPOVEC_USER: &str = "repovec";
/// The canonical `repovec` home directory.
pub const REPOVEC_HOME: &str = "/var/lib/repovec";
/// The canonical nologin shell.
pub const REPOVEC_SHELL: &str = "/usr/sbin/nologin";
