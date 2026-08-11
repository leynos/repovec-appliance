//! Validation helpers for the checked-in repovec directory-layout assets.
//!
//! This module belongs to [`crate::appliance`]. It embeds the repovec appliance
//! `tmpfiles.d` and `sysusers.d` assets at compile time with [`include_str!`]
//! and exposes the pure, I/O-free static validation surface for the directory
//! contract that governs the `repovec` system user's private on-disk tree.
//!
//! ## Validation Entry Points
//!
//! - [`validate_checked_in_directory_layout`] validates the two embedded
//!   packaging assets shipped in the repository.
//! - [`validate_directory_layout`] validates caller-supplied asset text. Use it
//!   in tests or tooling that needs to analyse asset contents sourced outside
//!   the checked-in files.
//!
//! ## Contract Scope
//!
//! The validator parses embedded asset text only (`include_str!`). It never
//! invokes `systemd-tmpfiles`, `systemd-sysusers`, `useradd`, `podman`, reads
//! `/etc/passwd`, or touches the live filesystem. Live ownership checks are the
//! responsibility of the separately carved-out pre-flight adapter.

mod error;
mod parser;

#[cfg(test)]
mod tests;
#[cfg(test)]
mod tests_proptest;

pub use error::{DirectoryLayoutError, Mode};

const CHECKED_IN_REPOVEC_TMPFILES: &str =
    include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/../../packaging/tmpfiles.d/repovec.conf"));
const CHECKED_IN_REPOVEC_SYSUSERS: &str =
    include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/../../packaging/sysusers.d/repovec.conf"));

/// The repository path of the checked-in `tmpfiles.d` asset.
pub const CHECKED_IN_REPOVEC_TMPFILES_PATH: &str = "packaging/tmpfiles.d/repovec.conf";
/// The repository path of the checked-in `sysusers.d` asset.
pub const CHECKED_IN_REPOVEC_SYSUSERS_PATH: &str = "packaging/sysusers.d/repovec.conf";
/// The installed path of the `tmpfiles.d` asset on a systemd host.
pub const INSTALLED_REPOVEC_TMPFILES_PATH: &str = "/usr/lib/tmpfiles.d/repovec.conf";
/// The installed path of the `sysusers.d` asset on a systemd host.
pub const INSTALLED_REPOVEC_SYSUSERS_PATH: &str = "/usr/lib/sysusers.d/repovec.conf";

/// Returns the repository's checked-in `tmpfiles.d` source.
///
/// # Examples
///
/// ```
/// use repovec_core::appliance::directory_layout::checked_in_repovec_tmpfiles;
///
/// assert!(checked_in_repovec_tmpfiles().contains("/var/lib/repovec"));
/// ```
#[must_use]
pub const fn checked_in_repovec_tmpfiles() -> &'static str { CHECKED_IN_REPOVEC_TMPFILES }

/// Returns the repository's checked-in `sysusers.d` source.
///
/// # Examples
///
/// ```
/// use repovec_core::appliance::directory_layout::checked_in_repovec_sysusers;
///
/// assert!(checked_in_repovec_sysusers().contains("repovec"));
/// ```
#[must_use]
pub const fn checked_in_repovec_sysusers() -> &'static str { CHECKED_IN_REPOVEC_SYSUSERS }

/// Validates the repository's checked-in directory-layout assets.
///
/// This checks the embedded `tmpfiles.d` data tree and `sysusers.d` declaration
/// against the appliance directory contract.
///
/// # Errors
///
/// Returns [`DirectoryLayoutError`] when a checked-in asset no longer satisfies
/// the appliance directory contract.
///
/// # Examples
///
/// ```
/// use repovec_core::appliance::directory_layout::validate_checked_in_directory_layout;
///
/// validate_checked_in_directory_layout().expect("the checked-in assets remain valid");
/// ```
pub fn validate_checked_in_directory_layout() -> Result<(), DirectoryLayoutError> {
    validate_directory_layout(checked_in_repovec_tmpfiles(), checked_in_repovec_sysusers())
}

/// Validates arbitrary repovec directory-layout asset contents.
///
/// # Errors
///
/// Returns [`DirectoryLayoutError`] describing the first contract violation.
///
/// # Examples
///
/// ```
/// use repovec_core::appliance::directory_layout::validate_directory_layout;
///
/// let tmpfiles = "\
/// # data tree
/// d /var/lib/repovec 0700 repovec repovec -
/// ";
/// let sysusers = "\
/// u repovec - \"repovec appliance service user\" /var/lib/repovec /usr/sbin/nologin
/// ";
/// validate_directory_layout(tmpfiles, sysusers).expect("the provided assets satisfy the contract");
/// ```
pub fn validate_directory_layout(
    _tmpfiles: &str,
    _sysusers: &str,
) -> Result<(), DirectoryLayoutError> {
    // Permissive stub: accept every asset so the contract tests can establish
    // their red state before the enforcement logic lands in Milestone 2.
    Ok(())
}
