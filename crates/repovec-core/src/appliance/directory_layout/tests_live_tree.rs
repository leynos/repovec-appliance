//! File-system cases for the live directory-layout pre-flight.
//!
//! The fixture tree is built with `cap_std` operations only, matching the
//! adapter itself and the `qdrant_liveness` test precedent, so the tests pass
//! the `no_std_fs_operations` Whitaker gate without allowances. Helpers return
//! `io::Result` so no `expect()` appears outside `#[test]` functions
//! (`no_expect_outside_tests`).
//!
//! A scratch tree cannot be chowned to root, so the on-disk cases seed all four
//! expectations to the test process's ids. That uniformity would hide a
//! mis-wired contract field, so two extra checks pin the mapping: the contract
//! table itself is asserted with distinct ids, and each directory family is
//! then re-verified with one expectation moved off the on-disk id.

use std::io;

use camino::Utf8PathBuf;
use cap_std::{
    ambient_authority,
    fs::{MetadataExt, PermissionsExt},
    fs_utf8::Dir,
};
use rstest::rstest;
use tempfile::TempDir;

use super::{
    directory_contract::DirectoryContract,
    live::{
        LiveLayoutError, LiveLayoutIds, layout_contracts, verify_layout_for_ids,
        verify_layout_from_databases,
    },
};
use crate::RuntimePaths;

const DATA_CHILDREN: [&str; 5] = ["", "git-mirrors", "worktrees", ".grepai", "qdrant-storage"];

/// A sentinel id that can never equal `id`.
///
/// `wrapping_add` is deliberate: a `saturating_sub(1).max(1)` sentinel collapses
/// onto the real id whenever the tree owner is uid or gid 1, which silently
/// turns the assertion into a no-op on such a host.
fn mismatched_id(id: u32) -> u32 { id.wrapping_add(1) }

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

/// Seeds every expectation to the ids the scratch tree actually carries.
fn live_ids(uid: u32, gid: u32) -> LiveLayoutIds {
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

/// The database-resolution seam must reach the directory checks.
///
/// This drives the production sequence end to end against captured database
/// text: the `passwd` record supplies the uid and the `group` row supplies the
/// gid. An unprivileged scratch tree cannot be root-owned, so the run must
/// fail on `qdrant-storage` with the root expectation. That failure is the
/// proof the chain ran: the resolved account ids were applied, and the
/// privileged directory was checked against root rather than against the
/// `repovec` ids.
#[test]
fn database_resolution_reaches_the_directory_checks() {
    let tree = LiveTree::new().expect("fixture tree");
    let (uid, gid) = current_ids(&tree).expect("data root ids");
    let passwd = format!("repovec:x:{uid}:{gid}:repovec:/var/lib/repovec:/usr/sbin/nologin\n");
    let group = format!("repovec:x:{gid}:\n");
    let storage = expected_absolute(&tree, "var/lib/repovec/qdrant-storage").expect("utf8 path");

    let result = verify_layout_from_databases(&tree.paths, &passwd, &group);

    let Err(LiveLayoutError::IncorrectOwner { violation }) = result else {
        panic!("expected the storage owner check to run, got {result:?}");
    };
    assert_eq!(violation.path, Utf8PathBuf::from(storage));
    assert_eq!(violation.expected, "root (uid 0)");
}

/// An unsafe account aborts resolution before any directory is statted.
///
/// A root-owned `repovec` account must stop the pre-flight even when the tree
/// itself is contract-correct, because every unit that runs as `User=repovec`
/// would otherwise run as root.
#[test]
fn database_resolution_rejects_an_unsafe_account_before_stating_any_directory() {
    let tree = LiveTree::new().expect("fixture tree");
    let passwd = "repovec:x:0:0:repovec:/var/lib/repovec:/usr/sbin/nologin\n";

    let result = verify_layout_from_databases(&tree.paths, passwd, "repovec:x:0:\n");

    assert!(matches!(result, Err(LiveLayoutError::RepovecUserIsRoot { uid: 0 })));
}

/// The contract must name the repovec ids for the data tree, root for
/// `qdrant-storage`, and root with the repovec group for `/etc/repovec`.
///
/// The fixture cannot chown to root, so this asserts the mapping directly on
/// the contract table. It is the check that fails if either privileged
/// directory is wired to the repovec ids, or if `qdrant-storage` is left in the
/// data-tree family.
#[test]
fn layout_contract_maps_each_directory_to_its_ids() {
    let tree = LiveTree::new().expect("fixture tree");
    let ids =
        LiveLayoutIds { repovec_uid: 998, repovec_gid: 996, privileged_uid: 0, privileged_gid: 0 };

    let contracts = layout_contracts(&tree.paths, ids);
    let contract_for = |path: &Utf8PathBuf| -> DirectoryContract {
        contracts
            .iter()
            .find_map(|(candidate, contract)| (candidate == path).then_some(*contract))
            .unwrap_or_else(|| panic!("no contract for {path}"))
    };

    let data_paths = [
        tree.paths.data_root().to_path_buf(),
        tree.paths.git_mirrors_root(),
        tree.paths.worktrees_root(),
        tree.paths.grepai_root(),
    ];
    for path in data_paths {
        let contract = contract_for(&path);
        assert_eq!((contract.owner_uid, contract.owner_gid), (998, 996), "data path {path}");
        assert_eq!(contract.mode, 0o700, "data path {path}");
    }

    let storage = contract_for(&tree.paths.qdrant_storage_root());
    assert_eq!((storage.owner_uid, storage.owner_gid), (0, 0), "qdrant-storage is root:root");
    assert_eq!(storage.mode, 0o700);

    let secrets = contract_for(&tree.paths.config_root().to_path_buf());
    assert_eq!((secrets.owner_uid, secrets.owner_gid), (0, 996), "/etc/repovec is root:repovec");
    assert_eq!(secrets.mode, 0o750);
}

/// Moving the privileged owner off the on-disk id must fail on
/// `qdrant-storage`, proving that directory consults the privileged uid.
#[test]
fn storage_directory_uses_the_privileged_owner() {
    let tree = LiveTree::new().expect("fixture tree");
    let (uid, gid) = current_ids(&tree).expect("data root ids");
    let mut ids = live_ids(uid, gid);
    ids.privileged_uid = mismatched_id(uid);
    let storage = expected_absolute(&tree, "var/lib/repovec/qdrant-storage").expect("utf8 path");

    let result = verify_layout_for_ids(&tree.paths, ids);

    let Err(LiveLayoutError::IncorrectOwner { violation }) = result else {
        panic!("expected the storage owner violation, got {result:?}");
    };
    assert_eq!(violation.path, Utf8PathBuf::from(storage));
    assert_eq!(violation.expected, format!("root (uid {})", ids.privileged_uid));
}

/// The same check for the privileged group.
#[test]
fn storage_directory_uses_the_privileged_group() {
    let tree = LiveTree::new().expect("fixture tree");
    let (uid, gid) = current_ids(&tree).expect("data root ids");
    let mut ids = live_ids(uid, gid);
    ids.privileged_gid = mismatched_id(gid);
    let storage = expected_absolute(&tree, "var/lib/repovec/qdrant-storage").expect("utf8 path");

    let result = verify_layout_for_ids(&tree.paths, ids);

    let Err(LiveLayoutError::IncorrectGroup { violation }) = result else {
        panic!("expected the storage group violation, got {result:?}");
    };
    assert_eq!(violation.path, Utf8PathBuf::from(storage));
    assert_eq!(violation.expected, format!("root (gid {})", ids.privileged_gid));
}

/// Moving the repovec owner off the on-disk id must fail on the data root,
/// proving it does not consult the privileged uid.
#[test]
fn data_tree_uses_the_repovec_owner() {
    let tree = LiveTree::new().expect("fixture tree");
    let (uid, gid) = current_ids(&tree).expect("data root ids");
    let mut ids = live_ids(uid, gid);
    ids.repovec_uid = mismatched_id(uid);
    let data_root = expected_absolute(&tree, "var/lib/repovec").expect("utf8 path");

    let result = verify_layout_for_ids(&tree.paths, ids);

    let Err(LiveLayoutError::IncorrectOwner { violation }) = result else {
        panic!("expected the data owner violation, got {result:?}");
    };
    assert_eq!(violation.path, Utf8PathBuf::from(data_root));
    assert_eq!(violation.expected, format!("repovec (uid {})", ids.repovec_uid));
}

/// The same check for the repovec group.
#[test]
fn data_tree_uses_the_repovec_group() {
    let tree = LiveTree::new().expect("fixture tree");
    let (uid, gid) = current_ids(&tree).expect("data root ids");
    let mut ids = live_ids(uid, gid);
    ids.repovec_gid = mismatched_id(gid);
    let data_root = expected_absolute(&tree, "var/lib/repovec").expect("utf8 path");

    let result = verify_layout_for_ids(&tree.paths, ids);

    let Err(LiveLayoutError::IncorrectGroup { violation }) = result else {
        panic!("expected the data group violation, got {result:?}");
    };
    assert_eq!(violation.path, Utf8PathBuf::from(data_root));
    assert_eq!(violation.expected, format!("repovec (gid {})", ids.repovec_gid));
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

/// A missing required directory is reported, distinguishing it from a
/// directory that exists but cannot be inspected.
#[rstest]
#[case::mode_too_permissive("var/lib/repovec/git-mirrors", 0o750, "0700", "0750")]
#[case::mode_too_restrictive("var/lib/repovec/git-mirrors", 0o500, "0700", "0500")]
#[case::secrets_world_writable("etc/repovec", 0o0777, "0750", "0777")]
#[case::set_group_id_bit("var/lib/repovec/git-mirrors", 0o2700, "0700", "2700")]
#[case::sticky_bit("var/lib/repovec/git-mirrors", 0o1700, "0700", "1700")]
fn incorrect_mode_is_reported(
    #[case] relative: &str,
    #[case] seeded_mode: u32,
    #[case] expected: &str,
    #[case] actual: &str,
) {
    let tree = LiveTree::new().expect("fixture tree");
    let (uid, gid) = current_ids(&tree).expect("data root ids");
    tree.chmod(relative, seeded_mode).expect("seed mode mismatch");

    let result = verify_layout_for_ids(&tree.paths, live_ids(uid, gid));

    let Err(LiveLayoutError::IncorrectMode { violation }) = result else {
        panic!("expected IncorrectMode, got {result:?}");
    };
    assert_eq!(
        violation.path,
        Utf8PathBuf::from(expected_absolute(&tree, relative).expect("utf8 path"))
    );
    assert_eq!(violation.expected, expected);
    assert_eq!(violation.actual, actual);
}

#[test]
fn symlink_in_place_of_a_directory_is_reported() {
    let tree = LiveTree::new().expect("fixture tree");
    let (uid, gid) = current_ids(&tree).expect("data root ids");
    let replaced = expected_absolute(&tree, "var/lib/repovec/worktrees").expect("utf8 path");
    tree.root.remove_dir_all("var/lib/repovec/worktrees").expect("remove worktrees");
    // The target is a contract-correct directory, so following the link would
    // satisfy every owner, group, and mode check while aliasing two required
    // paths onto one directory.
    tree.root
        .symlink("git-mirrors", "var/lib/repovec/worktrees")
        .expect("link worktrees to git-mirrors");

    let result = verify_layout_for_ids(&tree.paths, live_ids(uid, gid));

    let Err(LiveLayoutError::NotADirectory { path }) = result else {
        panic!("expected NotADirectory for a symlink, got {result:?}");
    };
    assert_eq!(path, Utf8PathBuf::from(replaced));
}

#[test]
fn regular_file_in_place_of_a_directory_is_reported() {
    let tree = LiveTree::new().expect("fixture tree");
    let (uid, gid) = current_ids(&tree).expect("data root ids");
    let replaced = expected_absolute(&tree, "var/lib/repovec/worktrees").expect("utf8 path");
    tree.root.remove_dir_all("var/lib/repovec/worktrees").expect("remove worktrees");
    // A file carrying the contract owner, group, and mode would satisfy every
    // id-and-mode check while leaving the documented directory unusable, so the
    // pre-flight must reject it on type alone.
    tree.root.write("var/lib/repovec/worktrees", b"not a directory").expect("write file");
    tree.chmod("var/lib/repovec/worktrees", 0o700).expect("chmod");

    let result = verify_layout_for_ids(&tree.paths, live_ids(uid, gid));

    let Err(LiveLayoutError::NotADirectory { path }) = result else {
        panic!("expected NotADirectory, got {result:?}");
    };
    assert_eq!(path, Utf8PathBuf::from(replaced));
}
