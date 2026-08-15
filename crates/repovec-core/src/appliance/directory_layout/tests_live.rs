//! Unit tests for the live directory-layout pre-flight adapter.
//!
//! The fixture tree is built with `cap_std` operations only, matching the
//! adapter itself and the `qdrant_liveness` test precedent, so the tests pass
//! the `no_std_fs_operations` Whitaker gate without allowances. Helpers return
//! `io::Result` so no `expect()` appears outside `#[test]` functions
//! (`no_expect_outside_tests`).

use std::io;

use camino::Utf8PathBuf;
use cap_std::{
    ambient_authority,
    fs::{MetadataExt as CapMetadataExt, PermissionsExt as CapPermissionsExt},
    fs_utf8::Dir,
};
use tempfile::TempDir;

use super::live::{LiveLayoutError, LiveLayoutIds, LiveLayoutViolation, verify_layout_for_ids};
use crate::RuntimePaths;

const DATA_CHILDREN: [&str; 5] = ["", "git-mirrors", "worktrees", ".grepai", "qdrant-storage"];

struct LiveTree {
    temp: TempDir,
    root: Dir,
    paths: RuntimePaths,
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

        // Provision the fixture tree with contract-correct modes so each test
        // seeds exactly one mismatch on top of an otherwise valid tree. The
        // data tree is private to repovec (SI-1/SI-2); /etc/repovec is
        // root-owned, non-world and non-group-write (SI-3).
        for child in DATA_CHILDREN {
            set_mode(&root, &format!("var/lib/repovec/{child}"), 0o700)?;
        }
        set_mode(&root, "etc/repovec", 0o750)?;

        let config_root = Utf8PathBuf::from_path_buf(temp.path().join("etc/repovec"))
            .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "config root is not UTF-8"))?;
        let data_root = Utf8PathBuf::from_path_buf(temp.path().join("var/lib/repovec"))
            .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "data root is not UTF-8"))?;
        let paths = RuntimePaths::new(config_root, data_root);

        Ok(Self { temp, root, paths })
    }

    fn chmod(&self, relative: &str, mode: u32) -> Result<(), io::Error> {
        set_mode(&self.root, relative, mode)
    }
}

fn set_mode(root: &Dir, relative: &str, mode: u32) -> Result<(), io::Error> {
    let permissions = cap_std::fs::Permissions::from_mode(mode);
    root.set_permissions(relative, permissions)
}

fn current_ids(tree: &LiveTree) -> Result<(u32, u32), io::Error> {
    let metadata = tree.root.metadata("var/lib/repovec")?;
    Ok((metadata.uid(), metadata.gid()))
}

fn live_ids(uid: u32, gid: u32) -> LiveLayoutIds {
    // A scratch tree is owned by the unprivileged test process, so the
    // privileged (root) expectation is also seeded to the owner for the
    // storage and secrets dirs; each test overrides what it wants to assert.
    LiveLayoutIds { repovec_uid: uid, repovec_gid: gid, privileged_uid: uid, privileged_gid: gid }
}

fn expected_absolute(tree: &LiveTree, relative: &str) -> Result<String, io::Error> {
    tree.temp
        .path()
        .join(relative)
        .to_str()
        .map(str::to_owned)
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "temp path is not UTF-8"))
}

#[test]
fn pass_path_accepts_a_correctly_provisioned_tree() {
    let tree = LiveTree::new().expect("fixture tree");
    let (uid, gid) = current_ids(&tree).expect("data root ids");

    let result = verify_layout_for_ids(&tree.paths, live_ids(uid, gid));
    assert!(result.is_ok(), "expected a correct tree to pass, got {result:?}");
}

#[test]
fn missing_data_directory_is_reported() {
    let tree = LiveTree::new().expect("fixture tree");
    let (uid, gid) = current_ids(&tree).expect("data root ids");
    let missing = expected_absolute(&tree, "var/lib/repovec/worktrees").expect("utf8 path");
    tree.root.remove_dir_all("var/lib/repovec/worktrees").expect("worktrees should be removed");

    let result = verify_layout_for_ids(&tree.paths, live_ids(uid, gid));

    let Err(LiveLayoutError::MissingPath { path }) = result else {
        panic!("expected MissingPath, got {result:?}");
    };
    assert_eq!(path, Utf8PathBuf::from(missing));
}

#[test]
fn over_permissive_data_directory_is_reported() {
    let tree = LiveTree::new().expect("fixture tree");
    let (uid, gid) = current_ids(&tree).expect("data root ids");
    tree.chmod("var/lib/repovec/git-mirrors", 0o750).expect("chmod");

    let result = verify_layout_for_ids(&tree.paths, live_ids(uid, gid));

    let Err(LiveLayoutError::InsecureMode { violation }) = result else {
        panic!("expected InsecureMode, got {result:?}");
    };
    assert_eq!(
        violation.path,
        Utf8PathBuf::from(
            expected_absolute(&tree, "var/lib/repovec/git-mirrors").expect("utf8 path")
        )
    );
}

#[test]
fn wrong_data_owner_is_reported() {
    let tree = LiveTree::new().expect("fixture tree");
    let (uid, gid) = current_ids(&tree).expect("data root ids");
    // A sentinel uid that cannot match the tree owner proves the owner check.
    let wrong_uid = uid.saturating_sub(1).max(1);

    let result = verify_layout_for_ids(&tree.paths, live_ids(wrong_uid, gid));

    let Err(LiveLayoutError::IncorrectOwner { violation }) = result else {
        panic!("expected IncorrectOwner, got {result:?}");
    };
    assert_eq!(
        violation.path,
        Utf8PathBuf::from(expected_absolute(&tree, "var/lib/repovec").expect("utf8 path"))
    );
}

#[test]
fn wrong_data_group_is_reported() {
    let tree = LiveTree::new().expect("fixture tree");
    let (uid, gid) = current_ids(&tree).expect("data root ids");
    let wrong_gid = gid.saturating_sub(1).max(1);

    let result = verify_layout_for_ids(&tree.paths, live_ids(uid, wrong_gid));

    let Err(LiveLayoutError::IncorrectGroup { violation }) = result else {
        panic!("expected IncorrectGroup, got {result:?}");
    };
    assert_eq!(
        violation.path,
        Utf8PathBuf::from(expected_absolute(&tree, "var/lib/repovec").expect("utf8 path"))
    );
}

#[test]
fn over_permissive_secrets_directory_is_reported() {
    let tree = LiveTree::new().expect("fixture tree");
    let (uid, gid) = current_ids(&tree).expect("data root ids");
    tree.chmod("etc/repovec", 0o0777).expect("chmod");

    let result = verify_layout_for_ids(&tree.paths, live_ids(uid, gid));

    let Err(LiveLayoutError::InsecureMode { violation }) = result else {
        panic!("expected InsecureMode, got {result:?}");
    };
    assert_eq!(
        violation.path,
        Utf8PathBuf::from(expected_absolute(&tree, "etc/repovec").expect("utf8 path"))
    );
}

#[test]
fn find_repovec_ids_parses_passwd_entry() {
    let sample = "root:x:0:0:root:/root:/bin/bash\nrepovec:x:998:996:repovec appliance service user:/var/lib/repovec:/usr/sbin/nologin\n";
    let ids = super::live::find_repovec_ids(sample);

    assert_eq!(ids, Some((998, 996)));
}

#[test]
fn find_repovec_ids_returns_none_when_absent() {
    let sample = "root:x:0:0:root:/root:/bin/bash\n";
    let ids = super::live::find_repovec_ids(sample);

    assert_eq!(ids, None);
}

#[test]
fn error_display_and_path_are_operator_facing() {
    let violation = LiveLayoutViolation {
        path: Utf8PathBuf::from("/var/lib/repovec"),
        expected: "uid 1000 (repovec)".to_owned(),
        actual: "uid 0".to_owned(),
    };
    let error = LiveLayoutError::IncorrectOwner { violation };

    assert_eq!(error.to_string(), "/var/lib/repovec must be owned by uid 1000 (repovec)");
    assert_eq!(error.path(), Some("/var/lib/repovec"));
}
