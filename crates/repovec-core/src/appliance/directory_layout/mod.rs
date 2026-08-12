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

use std::collections::HashSet;

use camino::Utf8PathBuf;
pub use error::{DirectoryLayoutError, Mode};

use crate::RuntimePaths;

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

/// The canonical `repovec` system user and group names.
pub const REPOVEC_USER: &str = "repovec";
/// The canonical `repovec` home directory.
pub const REPOVEC_HOME: &str = "/var/lib/repovec";
/// The canonical nologin shell.
pub const REPOVEC_SHELL: &str = "/usr/sbin/nologin";

/// A single directory-entry expectation in the layout contract.
#[derive(Clone, Debug, Eq, PartialEq)]
struct DirectorySpec {
    path: Utf8PathBuf,
    mode: Mode,
    owner: &'static str,
    group: &'static str,
}

impl DirectorySpec {
    fn new(
        path: impl Into<Utf8PathBuf>,
        mode: Mode,
        owner: &'static str,
        group: &'static str,
    ) -> Self {
        Self { path: path.into(), mode, owner, group }
    }
}

/// Returns the directory-layout contract for the appliance runtime paths.
fn layout_contract(paths: &RuntimePaths) -> Vec<DirectorySpec> {
    vec![
        DirectorySpec::new(paths.data_root(), Mode(0o700), REPOVEC_USER, REPOVEC_USER),
        DirectorySpec::new(paths.git_mirrors_root(), Mode(0o700), REPOVEC_USER, REPOVEC_USER),
        DirectorySpec::new(paths.worktrees_root(), Mode(0o700), REPOVEC_USER, REPOVEC_USER),
        DirectorySpec::new(paths.grepai_root(), Mode(0o700), REPOVEC_USER, REPOVEC_USER),
        DirectorySpec::new(paths.qdrant_storage_root(), Mode(0o700), "root", "root"),
    ]
}

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
/// // A data tree missing the worktrees entry violates the contract.
/// let tmpfiles = "\
/// d /var/lib/repovec 0700 repovec repovec -\
/// d /var/lib/repovec/git-mirrors 0700 repovec repovec -\
/// d /var/lib/repovec/.grepai 0700 repovec repovec -\
/// d /var/lib/repovec/qdrant-storage 0700 root root -
/// ";
/// let sysusers = "\
/// u repovec - \"repovec appliance service user\" /var/lib/repovec /usr/sbin/nologin
/// ";
/// assert!(validate_directory_layout(tmpfiles, sysusers).is_err());
/// ```
pub fn validate_directory_layout(
    tmpfiles: &str,
    sysusers: &str,
) -> Result<(), DirectoryLayoutError> {
    let paths = RuntimePaths::appliance_defaults();
    let contract = layout_contract(&paths);

    validate_tmpfiles_entries(tmpfiles, &paths, &contract)?;
    validate_sysusers_entries(sysusers)
}

fn validate_tmpfiles_entries(
    tmpfiles: &str,
    paths: &RuntimePaths,
    contract: &[DirectorySpec],
) -> Result<(), DirectoryLayoutError> {
    let mut expected_paths = contract.iter().map(|spec| spec.path.as_str()).collect::<HashSet<_>>();

    for (line_index, raw_line) in tmpfiles.lines().enumerate() {
        let line_number = line_index + 1;
        let Some(entry) = parser::tmpfiles_entry(raw_line, "tmpfiles.d/repovec.conf", line_number)?
        else {
            continue;
        };

        let path = entry.path.as_str();
        let Some(spec) = contract.iter().find(|spec| spec.path.as_str() == path) else {
            // A config-root entry violates the single-authority invariant (SI-4):
            // the libexec helper owns /etc/repovec, so the tmpfiles asset must
            // never declare it. Anything else outside the contract is just an
            // unexpected (undeclared) directory.
            return Err(unexpected_tmpfiles_path_error(path, paths));
        };

        compare_spec(spec, &entry, line_number)?;
        expected_paths.remove(path);
    }

    if let Some(path) = expected_paths.into_iter().next() {
        return Err(DirectoryLayoutError::MissingDirectoryEntry { path: Utf8PathBuf::from(path) });
    }

    Ok(())
}

fn unexpected_tmpfiles_path_error(
    path: &str,
    runtime_paths: &RuntimePaths,
) -> DirectoryLayoutError {
    if path == runtime_paths.config_root().as_str()
        || path.starts_with(runtime_paths.config_root().as_str())
    {
        DirectoryLayoutError::ForbiddenSecretsEntry { path: Utf8PathBuf::from(path) }
    } else {
        DirectoryLayoutError::UnexpectedDirectoryEntry { path: Utf8PathBuf::from(path) }
    }
}

fn compare_spec(
    spec: &DirectorySpec,
    entry: &parser::TmpfilesEntry,
    line_number: usize,
) -> Result<(), DirectoryLayoutError> {
    let actual_mode =
        parser::parse_mode(&entry.mode).ok_or(DirectoryLayoutError::NonExplicitField {
            asset: "tmpfiles.d/repovec.conf",
            line_number,
            field: "mode",
        })?;

    if actual_mode != spec.mode {
        return Err(DirectoryLayoutError::IncorrectMode {
            path: Utf8PathBuf::from(entry.path.as_str()),
            expected: spec.mode,
            actual: actual_mode,
        });
    }

    if entry.user != spec.owner {
        return Err(DirectoryLayoutError::IncorrectOwner {
            path: Utf8PathBuf::from(entry.path.as_str()),
            expected: spec.owner,
            actual: entry.user.clone(),
        });
    }

    if entry.group != spec.group {
        return Err(DirectoryLayoutError::IncorrectGroup {
            path: Utf8PathBuf::from(entry.path.as_str()),
            expected: spec.group,
            actual: entry.group.clone(),
        });
    }

    Ok(())
}

fn validate_sysusers_entries(sysusers: &str) -> Result<(), DirectoryLayoutError> {
    let mut found_user = false;

    for (line_index, raw_line) in sysusers.lines().enumerate() {
        let line_number = line_index + 1;
        let Some(entry) =
            parser::sysusers_user_line(raw_line, "sysusers.d/repovec.conf", line_number)?
        else {
            continue;
        };

        if entry.name != REPOVEC_USER {
            return Err(DirectoryLayoutError::SysusersMissingUser);
        }

        found_user = true;

        if entry.home != REPOVEC_HOME {
            return Err(DirectoryLayoutError::SysusersIncorrectHome {
                expected: REPOVEC_HOME,
                actual: entry.home.clone(),
            });
        }

        if entry.shell != REPOVEC_SHELL {
            return Err(DirectoryLayoutError::SysusersIncorrectShell {
                expected: REPOVEC_SHELL,
                actual: entry.shell.clone(),
            });
        }
    }

    if !found_user {
        return Err(DirectoryLayoutError::SysusersMissingUser);
    }

    Ok(())
}
