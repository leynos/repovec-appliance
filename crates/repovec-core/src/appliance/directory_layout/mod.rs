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

mod directory_contract;
mod error;
mod group;
pub mod live;
mod parser;

#[cfg(test)]
mod tests;
#[cfg(test)]
mod tests_display;
#[cfg(test)]
mod tests_live;
#[cfg(test)]
mod tests_proptest;
#[cfg(test)]
mod tests_support;

use camino::{Utf8Path, Utf8PathBuf};
pub use error::{DirectoryLayoutError, Mode};
use error::{SYSUSERS_ASSET, TMPFILES_ASSET};

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
/// let tmpfiles = concat!(
///     "d /var/lib/repovec 0700 repovec repovec -\n",
///     "d /var/lib/repovec/git-mirrors 0700 repovec repovec -\n",
///     "d /var/lib/repovec/.grepai 0700 repovec repovec -\n",
///     "d /var/lib/repovec/qdrant-storage 0700 root root -\n",
/// );
/// let sysusers = concat!(
///     "u repovec - \"repovec appliance service user\" /var/lib/repovec /usr/sbin/nologin\n",
/// );
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
    let mut satisfied = vec![false; contract.len()];

    for (line_index, raw_line) in tmpfiles.lines().enumerate() {
        let line_number = line_index + 1;
        let Some(entry) = parser::tmpfiles_entry(raw_line, TMPFILES_ASSET, line_number)? else {
            continue;
        };

        let path = entry.path.as_str();
        let Some((index, spec)) =
            contract.iter().enumerate().find(|(_, spec)| spec.path.as_str() == path)
        else {
            // A config-root entry violates the single-authority invariant (SI-4):
            // the libexec helper owns /etc/repovec, so the tmpfiles asset must
            // never declare it. Anything else outside the contract is just an
            // unexpected (undeclared) directory.
            return Err(unexpected_tmpfiles_path_error(path, paths));
        };

        compare_spec(spec, &entry, line_number)?;

        // `index` comes from enumerating the contract, and `satisfied` is
        // allocated with exactly the contract's length, so the slot exists.
        // `get_mut` is used rather than indexing so a future change to either
        // length becomes a visible no-op instead of a panic.
        if let Some(is_satisfied) = satisfied.get_mut(index) {
            *is_satisfied = true;
        }
    }

    // Report the first contract entry with no asset line, searching in contract
    // order rather than in hash order. The operator sees the same missing path
    // on every run, and it is the first one in the documented layout.
    for (spec, is_satisfied) in contract.iter().zip(&satisfied) {
        if !is_satisfied {
            return Err(DirectoryLayoutError::MissingDirectoryEntry { path: spec.path.clone() });
        }
    }

    Ok(())
}

fn unexpected_tmpfiles_path_error(
    path: &str,
    runtime_paths: &RuntimePaths,
) -> DirectoryLayoutError {
    // Compare path components, not string prefixes: `/etc/repovec-backup` is a
    // different directory, and a prefix comparison would misreport it as a
    // second authority for the secrets file.
    let candidate = Utf8Path::new(path);
    if candidate.starts_with(runtime_paths.config_root()) {
        DirectoryLayoutError::ForbiddenSecretsEntry { path: candidate.to_path_buf() }
    } else {
        DirectoryLayoutError::UnexpectedDirectoryEntry { path: candidate.to_path_buf() }
    }
}

fn compare_spec(
    spec: &DirectorySpec,
    entry: &parser::TmpfilesEntry,
    line_number: usize,
) -> Result<(), DirectoryLayoutError> {
    let actual_mode = parser::parse_mode(&entry.mode).map_err(|error| match error {
        parser::ModeParseError::Placeholder => DirectoryLayoutError::NonExplicitField {
            asset: TMPFILES_ASSET,
            line_number,
            field: "mode",
        },
        parser::ModeParseError::Invalid => DirectoryLayoutError::InvalidField {
            asset: TMPFILES_ASSET,
            line_number,
            field: "mode",
            value: entry.mode.clone(),
        },
    })?;

    compare_spec_against(spec, entry, actual_mode)
}

/// One field of a [`DirectorySpec`], expressed as data.
///
/// The mode, owner, and group comparisons share a single decision—does the
/// parsed entry satisfy the contract, and if not, what did the contract require
/// against what the asset declares—and differ only in the predicate and the
/// error variant. Holding those together lets the three run as one ordered pass
/// rather than three near-identical functions.
#[derive(Clone, Copy)]
enum SpecField {
    Owner,
    Group,
    Mode,
}

impl SpecField {
    /// Whether `entry` satisfies this field of `spec`.
    fn is_satisfied(
        self,
        spec: &DirectorySpec,
        entry: &parser::TmpfilesEntry,
        actual_mode: Mode,
    ) -> bool {
        match self {
            Self::Owner => entry.user == spec.owner,
            Self::Group => entry.group == spec.group,
            Self::Mode => actual_mode == spec.mode,
        }
    }

    /// The violation raised when this field of `spec` is not satisfied.
    fn violation(
        self,
        spec: &DirectorySpec,
        entry: &parser::TmpfilesEntry,
        actual_mode: Mode,
    ) -> DirectoryLayoutError {
        let path = Utf8PathBuf::from(entry.path.as_str());
        match self {
            Self::Owner => DirectoryLayoutError::IncorrectOwner {
                path,
                expected: spec.owner,
                actual: entry.user.clone(),
            },
            Self::Group => DirectoryLayoutError::IncorrectGroup {
                path,
                expected: spec.group,
                actual: entry.group.clone(),
            },
            Self::Mode => DirectoryLayoutError::IncorrectMode {
                path,
                expected: spec.mode,
                actual: actual_mode,
            },
        }
    }
}

/// The compared fields, in the order their violations are reported.
///
/// The order is the contract: the validator returns the first unsatisfied field,
/// so the list runs from the least specific mismatch to the most.
const COMPARED_FIELDS: [SpecField; 3] = [SpecField::Owner, SpecField::Group, SpecField::Mode];

/// Compares a parsed entry against a directory spec.
///
/// `actual_mode` is the mode column already parsed by [`compare_spec`], which is
/// what keeps the mode comparison total: the explicitness and syntax failures
/// have been reported before this runs.
fn compare_spec_against(
    spec: &DirectorySpec,
    entry: &parser::TmpfilesEntry,
    actual_mode: Mode,
) -> Result<(), DirectoryLayoutError> {
    COMPARED_FIELDS.into_iter().try_for_each(|field| {
        if field.is_satisfied(spec, entry, actual_mode) {
            Ok(())
        } else {
            Err(field.violation(spec, entry, actual_mode))
        }
    })
}

fn validate_sysusers_entries(sysusers: &str) -> Result<(), DirectoryLayoutError> {
    let mut found_user = false;

    for (line_index, raw_line) in sysusers.lines().enumerate() {
        let line_number = line_index + 1;
        let Some(entry) = parser::sysusers_user_line(raw_line, SYSUSERS_ASSET, line_number)? else {
            continue;
        };

        // Another account declared by the same asset is not the repovec user and
        // is simply not the entry this validator is looking for; only the
        // absence of a repovec declaration is a failure.
        if entry.name != REPOVEC_USER {
            continue;
        }

        found_user = true;

        // systemd-sysusers allocates the uid when the column is `-`. Pinning a
        // numeric uid would make provisioning depend on that number being free
        // on the host, which the appliance does not control.
        if entry.uid != "-" {
            return Err(DirectoryLayoutError::SysusersPinnedUid { actual: entry.uid.clone() });
        }

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

    if found_user { Ok(()) } else { Err(DirectoryLayoutError::SysusersMissingUser) }
}
