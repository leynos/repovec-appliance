//! File-system cases for the live directory-layout pre-flight.
//!
//! The fixture tree and identity helpers live in
//! [`super::tests_live_tree_support`]; this module holds only the cases.
//!
//! A scratch tree cannot be chowned to root, so the on-disk cases seed all four
//! expectations to the test process's ids. That uniformity would hide a
//! mis-wired contract field, so two extra checks pin the mapping: the contract
//! table itself is asserted with distinct ids, and each directory family is
//! then re-verified with one expectation moved off the on-disk id.

use std::io;

use camino::Utf8PathBuf;
use rstest::rstest;

use super::{
    directory_contract::DirectoryContract,
    live::{
        LiveLayoutError, LiveLayoutIds, layout_contracts, verify_layout_for_ids,
        verify_layout_from_databases,
    },
    tests_live_tree_support::{
        IdentityExpectation, LiveTree, current_ids, expected_absolute, live_ids, live_tree,
    },
};

#[rstest]
fn pass_path_accepts_a_correctly_provisioned_tree(live_tree: Result<LiveTree, io::Error>) {
    let tree = live_tree.expect("fixture tree");
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
#[rstest]
fn database_resolution_reaches_the_directory_checks(live_tree: Result<LiveTree, io::Error>) {
    let tree = live_tree.expect("fixture tree");
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
#[rstest]
fn database_resolution_rejects_an_unsafe_account_before_stating_any_directory(
    live_tree: Result<LiveTree, io::Error>,
) {
    let tree = live_tree.expect("fixture tree");
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
#[rstest]
fn layout_contract_maps_each_directory_to_its_ids(live_tree: Result<LiveTree, io::Error>) {
    let tree = live_tree.expect("fixture tree");
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

/// Keep the four runtime identity mappings independently regression-tested.
#[rstest]
#[case::repovec_uid(IdentityExpectation::RepovecUid)]
#[case::repovec_gid(IdentityExpectation::RepovecGid)]
#[case::privileged_uid(IdentityExpectation::PrivilegedUid)]
#[case::privileged_gid(IdentityExpectation::PrivilegedGid)]
fn each_directory_family_uses_its_expected_identity(
    #[case] expectation: IdentityExpectation,
    live_tree: Result<LiveTree, io::Error>,
) {
    let tree = live_tree.expect("fixture tree");
    let (uid, gid) = current_ids(&tree).expect("data root ids");
    let mut ids = live_ids(uid, gid);
    expectation.displace(&mut ids, uid, gid);
    let expected_path = expected_absolute(&tree, expectation.relative_path()).expect("utf8 path");

    let result = verify_layout_for_ids(&tree.paths, ids);

    let Err(error) = result else {
        panic!("expected an identity violation, got {result:?}");
    };
    assert_eq!(
        matches!(&error, LiveLayoutError::IncorrectGroup { .. }),
        expectation.is_group(),
        "identity violation kind should match the displaced contract field",
    );
    let violation = match error {
        LiveLayoutError::IncorrectOwner { violation }
        | LiveLayoutError::IncorrectGroup { violation } => violation,
        other => panic!("expected an identity violation, got {other:?}"),
    };
    assert_eq!(violation.path, Utf8PathBuf::from(expected_path));
    assert_eq!(violation.expected, expectation.expected_description(&ids));
}

#[rstest]
fn missing_data_directory_is_reported(live_tree: Result<LiveTree, io::Error>) {
    let tree = live_tree.expect("fixture tree");
    let (uid, gid) = current_ids(&tree).expect("data root ids");
    let missing = expected_absolute(&tree, "var/lib/repovec/worktrees").expect("utf8 path");
    tree.root.remove_dir_all("var/lib/repovec/worktrees").expect("worktrees should be removed");

    let result = verify_layout_for_ids(&tree.paths, live_ids(uid, gid));

    let Err(LiveLayoutError::MissingPath { path }) = result else {
        panic!("expected MissingPath, got {result:?}");
    };
    assert_eq!(path, Utf8PathBuf::from(missing));
}

/// One seeded mode mismatch, named so the case table reads as data.
struct ModeMismatchCase {
    relative: &'static str,
    seeded_mode: u32,
    expected_mode: &'static str,
    actual_mode: &'static str,
}

/// A mode mismatch on a required directory is reported with both spellings.
#[rstest]
#[case::mode_too_permissive(ModeMismatchCase {
    relative: "var/lib/repovec/git-mirrors",
    seeded_mode: 0o750,
    expected_mode: "0700",
    actual_mode: "0750",
})]
#[case::mode_too_restrictive(ModeMismatchCase {
    relative: "var/lib/repovec/git-mirrors",
    seeded_mode: 0o500,
    expected_mode: "0700",
    actual_mode: "0500",
})]
#[case::secrets_world_writable(ModeMismatchCase {
    relative: "etc/repovec",
    seeded_mode: 0o0777,
    expected_mode: "0750",
    actual_mode: "0777",
})]
#[case::set_group_id_bit(ModeMismatchCase {
    relative: "var/lib/repovec/git-mirrors",
    seeded_mode: 0o2700,
    expected_mode: "0700",
    actual_mode: "2700",
})]
#[case::sticky_bit(ModeMismatchCase {
    relative: "var/lib/repovec/git-mirrors",
    seeded_mode: 0o1700,
    expected_mode: "0700",
    actual_mode: "1700",
})]
fn incorrect_mode_is_reported(
    #[case] case: ModeMismatchCase,
    live_tree: Result<LiveTree, io::Error>,
) {
    let tree = live_tree.expect("fixture tree");
    let (uid, gid) = current_ids(&tree).expect("data root ids");
    tree.chmod(case.relative, case.seeded_mode).expect("seed mode mismatch");

    let result = verify_layout_for_ids(&tree.paths, live_ids(uid, gid));

    let Err(LiveLayoutError::IncorrectMode { violation }) = result else {
        panic!("expected IncorrectMode, got {result:?}");
    };
    assert_eq!(
        violation.path,
        Utf8PathBuf::from(expected_absolute(&tree, case.relative).expect("utf8 path"))
    );
    assert_eq!(violation.expected, case.expected_mode);
    assert_eq!(violation.actual, case.actual_mode);
}

#[rstest]
fn symlink_in_place_of_a_directory_is_reported(live_tree: Result<LiveTree, io::Error>) {
    let tree = live_tree.expect("fixture tree");
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

#[rstest]
fn regular_file_in_place_of_a_directory_is_reported(live_tree: Result<LiveTree, io::Error>) {
    let tree = live_tree.expect("fixture tree");
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
