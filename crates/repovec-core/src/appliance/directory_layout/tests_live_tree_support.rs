//! Fixtures and helpers shared by the live directory-layout tests.
//!
//! The fixture tree is built with `cap_std` operations only, matching the
//! adapter itself and the `qdrant_liveness` test precedent, so the tests pass
//! the `no_std_fs_operations` Whitaker gate without allowances. Helpers and
//! fixtures return `io::Result` so no `expect()` appears outside a test body
//! (`no_expect_outside_tests`).
//!
//! The cases that use these fixtures live in [`super::tests_live_tree`]; they
//! are kept apart because Whitaker's `module-max-lines` limit counts each
//! module file separately.

use std::io;

use camino::Utf8PathBuf;
use cap_std::{
    ambient_authority,
    fs::{MetadataExt, PermissionsExt},
    fs_utf8::Dir,
};
use rstest::fixture;
use tempfile::TempDir;

use super::live::LiveLayoutIds;
use crate::RuntimePaths;

pub(super) const DATA_CHILDREN: [&str; 5] =
    ["", "git-mirrors", "worktrees", ".grepai", "qdrant-storage"];

/// One of the four identity fields the live pre-flight checks.
#[derive(Clone, Copy)]
pub(super) enum IdentityExpectation {
    RepovecUid,
    RepovecGid,
    PrivilegedUid,
    PrivilegedGid,
}

impl IdentityExpectation {
    pub(super) fn relative_path(self) -> &'static str {
        match self {
            Self::RepovecUid | Self::RepovecGid => "var/lib/repovec",
            Self::PrivilegedUid | Self::PrivilegedGid => "var/lib/repovec/qdrant-storage",
        }
    }

    pub(super) fn expected_description(self, ids: &LiveLayoutIds) -> String {
        match self {
            Self::RepovecUid => format!("repovec (uid {})", ids.repovec_uid),
            Self::RepovecGid => format!("repovec (gid {})", ids.repovec_gid),
            Self::PrivilegedUid => format!("root (uid {})", ids.privileged_uid),
            Self::PrivilegedGid => format!("root (gid {})", ids.privileged_gid),
        }
    }

    pub(super) fn is_group(self) -> bool { matches!(self, Self::RepovecGid | Self::PrivilegedGid) }

    pub(super) fn displace(self, ids: &mut LiveLayoutIds, uid: u32, gid: u32) {
        match self {
            Self::RepovecUid => ids.repovec_uid = mismatched_id(uid),
            Self::RepovecGid => ids.repovec_gid = mismatched_id(gid),
            Self::PrivilegedUid => ids.privileged_uid = mismatched_id(uid),
            Self::PrivilegedGid => ids.privileged_gid = mismatched_id(gid),
        }
    }
}

/// A sentinel id that can never equal `id`.
///
/// `wrapping_add` is deliberate: a `saturating_sub(1).max(1)` sentinel collapses
/// onto the real id whenever the tree owner is uid or gid 1, which silently
/// turns the assertion into a no-op on such a host.
fn mismatched_id(id: u32) -> u32 { id.wrapping_add(1) }

/// A scratch tree provisioned to the live-layout contract.
pub(super) struct LiveTree {
    temp: TempDir,
    pub(super) root: Dir,
    pub(super) paths: RuntimePaths,
}

impl LiveTree {
    fn new() -> Result<Self, io::Error> {
        let temp = tempfile::tempdir()?;
        let root_path = temp
            .path()
            .to_str()
            .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "temp path is not UTF-8"))?;
        let root = Dir::open_ambient_dir(root_path, ambient_authority())?;

        for child in DATA_CHILDREN {
            root.create_dir_all(format!("var/lib/repovec/{child}"))?;
        }
        root.create_dir_all("etc/repovec")?;

        // Provision the tree with contract-correct modes so each test seeds
        // exactly one mismatch on an otherwise valid fixture.
        for child in DATA_CHILDREN {
            set_mode(&root, &format!("var/lib/repovec/{child}"), 0o700)?;
        }
        set_mode(&root, "etc/repovec", 0o750)?;

        let config_root = Utf8PathBuf::from_path_buf(temp.path().join("etc/repovec"))
            .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "config root is not UTF-8"))?;
        let data_root = Utf8PathBuf::from_path_buf(temp.path().join("var/lib/repovec"))
            .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "data root is not UTF-8"))?;

        Ok(Self { temp, root, paths: RuntimePaths::new(config_root, data_root) })
    }

    pub(super) fn chmod(&self, relative: &str, mode: u32) -> Result<(), io::Error> {
        set_mode(&self.root, relative, mode)
    }
}

/// Builds a scratch tree provisioned to the live-layout contract.
///
/// The fixture returns `io::Result` so the filesystem work stays fallible; each
/// test body turns that error into a panic at the point of use.
#[fixture]
#[rustfmt::skip]
pub(super) fn live_tree() -> Result<LiveTree, io::Error> {
    // Rustfmt collapsing this body to one line triggers `unused-braces`; keep an
    // explicit block despite the terse body.
    LiveTree::new()
}

fn set_mode(root: &Dir, relative: &str, mode: u32) -> Result<(), io::Error> {
    let permissions = cap_std::fs::Permissions::from_mode(mode);
    root.set_permissions(relative, permissions)
}

pub(super) fn current_ids(tree: &LiveTree) -> Result<(u32, u32), io::Error> {
    let metadata = tree.root.metadata("var/lib/repovec")?;
    Ok((metadata.uid(), metadata.gid()))
}

/// Seeds every expectation to the ids the scratch tree actually carries.
pub(super) fn live_ids(uid: u32, gid: u32) -> LiveLayoutIds {
    LiveLayoutIds { repovec_uid: uid, repovec_gid: gid, privileged_uid: uid, privileged_gid: gid }
}

pub(super) fn expected_absolute(tree: &LiveTree, relative: &str) -> Result<String, io::Error> {
    tree.temp
        .path()
        .join(relative)
        .to_str()
        .map(str::to_owned)
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "temp path is not UTF-8"))
}
