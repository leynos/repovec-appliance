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
use rstest::rstest;
use tempfile::TempDir;

use super::{
    group::find_group_gid,
    live::{LiveLayoutError, LiveLayoutIds, LiveLayoutViolation, verify_layout_for_ids},
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

/// Cases where a directory's on-disk state differs from the contract, and the
/// violation the adapter must report for it.
///
/// Each case runs the same path—build a correct tree, seed one mismatch, run the
/// pre-flight—so the cases are held as data and driven through one test body.
#[derive(Clone, Copy, Debug)]
enum LiveViolationCase {
    /// `var/lib/repovec/git-mirrors` is widened to group-readable.
    OverPermissiveMode,
    /// `var/lib/repovec/git-mirrors` is more restrictive than the contract.
    ///
    /// The mode comparison is exact, not one-directional: a directory more
    /// restrictive than the contract is as unusable to the daemon as one that is
    /// too permissive, so both are rejected.
    OverRestrictiveMode,
    /// `etc/repovec` is world-writable.
    OverPermissiveSecretsMode,
    /// `var/lib/repovec/git-mirrors` carries the contracted `0700` plus set-group-ID.
    ///
    /// The nine ordinary bits match the contract exactly, so this case only
    /// fails if the live check compares the special bits too.
    SetGroupIdBitSet,
    /// `var/lib/repovec/git-mirrors` carries the contracted `0700` plus sticky bit.
    StickyBitSet,
    /// The data root is expected to belong to a uid it does not have.
    WrongOwner,
    /// The data root is expected to belong to a gid it does not have.
    WrongGroup,
}

impl LiveViolationCase {
    /// Seeds this case's single mismatch on an otherwise contract-correct tree.
    fn seed(self, tree: &LiveTree) {
        match self {
            Self::OverPermissiveMode => {
                tree.chmod("var/lib/repovec/git-mirrors", 0o750)
                    .expect("chmod git-mirrors to 0750");
            }
            Self::OverRestrictiveMode => {
                tree.chmod("var/lib/repovec/git-mirrors", 0o500)
                    .expect("chmod git-mirrors to 0500");
            }
            Self::OverPermissiveSecretsMode => {
                tree.chmod("etc/repovec", 0o0777).expect("chmod etc/repovec to 0777");
            }
            Self::SetGroupIdBitSet => {
                tree.chmod("var/lib/repovec/git-mirrors", 0o2700)
                    .expect("chmod git-mirrors to 2700");
            }
            Self::StickyBitSet => {
                tree.chmod("var/lib/repovec/git-mirrors", 0o1700)
                    .expect("chmod git-mirrors to 1700");
            }
            Self::WrongOwner | Self::WrongGroup => {}
        }
    }

    /// The expected ids for this case, seeded so exactly one of them is wrong.
    fn ids(self, uid: u32, gid: u32) -> LiveLayoutIds {
        match self {
            Self::WrongOwner => live_ids(mismatched_id(uid), gid),
            Self::WrongGroup => live_ids(uid, mismatched_id(gid)),
            _ => live_ids(uid, gid),
        }
    }

    /// The directory this case expects the violation to name.
    fn violated_path(self) -> &'static str {
        match self {
            Self::OverPermissiveMode
            | Self::OverRestrictiveMode
            | Self::SetGroupIdBitSet
            | Self::StickyBitSet => "var/lib/repovec/git-mirrors",
            Self::OverPermissiveSecretsMode => "etc/repovec",
            Self::WrongOwner | Self::WrongGroup => "var/lib/repovec",
        }
    }
}

#[rstest]
#[case::over_permissive_data_directory(LiveViolationCase::OverPermissiveMode, "0700", "0750")]
#[case::over_restrictive_data_directory(LiveViolationCase::OverRestrictiveMode, "0700", "0500")]
#[case::over_permissive_secrets_directory(
    LiveViolationCase::OverPermissiveSecretsMode,
    "0750",
    "0777"
)]
#[case::set_group_id_bit_on_data_directory(LiveViolationCase::SetGroupIdBitSet, "0700", "2700")]
#[case::sticky_bit_on_data_directory(LiveViolationCase::StickyBitSet, "0700", "1700")]
fn incorrect_mode_is_reported(
    #[case] case: LiveViolationCase,
    #[case] expected: &str,
    #[case] actual: &str,
) {
    let tree = LiveTree::new().expect("fixture tree");
    let (uid, gid) = current_ids(&tree).expect("data root ids");
    case.seed(&tree);

    let result = verify_layout_for_ids(&tree.paths, case.ids(uid, gid));

    let Err(LiveLayoutError::IncorrectMode { violation }) = result else {
        panic!("expected IncorrectMode, got {result:?}");
    };
    assert_eq!(
        violation.path,
        Utf8PathBuf::from(expected_absolute(&tree, case.violated_path()).expect("utf8 path"))
    );
    assert_eq!(violation.expected, expected);
    assert_eq!(violation.actual, actual);
}

#[rstest]
#[case::wrong_data_owner(LiveViolationCase::WrongOwner)]
#[case::wrong_data_group(LiveViolationCase::WrongGroup)]
fn incorrect_identity_is_reported(#[case] case: LiveViolationCase) {
    let tree = LiveTree::new().expect("fixture tree");
    let (uid, gid) = current_ids(&tree).expect("data root ids");

    let outcome = verify_layout_for_ids(&tree.paths, case.ids(uid, gid));

    let violation = match (case, outcome) {
        (LiveViolationCase::WrongOwner, Err(LiveLayoutError::IncorrectOwner { violation }))
        | (LiveViolationCase::WrongGroup, Err(LiveLayoutError::IncorrectGroup { violation })) => {
            violation
        }
        (observed, actual_outcome) => {
            panic!("expected an identity violation for {observed:?}, got {actual_outcome:?}")
        }
    };

    assert_eq!(
        violation.path,
        Utf8PathBuf::from(expected_absolute(&tree, case.violated_path()).expect("utf8 path"))
    );

    // The sentinel id proves the check actually ran: the expectation names the id
    // the pre-flight was asked to require, and the report names the id on disk.
    let (expected, actual) = match case {
        LiveViolationCase::WrongOwner => {
            (format!("repovec (uid {})", mismatched_id(uid)), format!("uid {uid}"))
        }
        LiveViolationCase::WrongGroup => {
            (format!("repovec (gid {})", mismatched_id(gid)), format!("gid {gid}"))
        }
        other => panic!("{other:?} does not seed an identity mismatch"),
    };
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

#[test]
fn find_group_gid_parses_group_entry() {
    let sample = "root:x:0:\nwheel:x:10:leynos\nrepovec:x:996:\n";

    assert_eq!(find_group_gid(sample, "repovec"), Some(996));
}

#[test]
fn find_group_gid_is_independent_of_row_order() {
    let sample = "repovec:x:996:\nroot:x:0:\n";

    assert_eq!(find_group_gid(sample, "repovec"), Some(996));
}

#[test]
fn find_group_gid_returns_none_when_absent() {
    let sample = "root:x:0:\nwheel:x:10:leynos\n";

    assert_eq!(find_group_gid(sample, "repovec"), None);
}

#[test]
fn find_group_gid_ignores_a_prefix_match_on_the_name() {
    // `repovec-backup` contains `repovec` as a prefix; the lookup compares the
    // whole name column so the second group cannot be mistaken for the first.
    let sample = "repovec-backup:x:997:\nrepovec:x:996:\n";

    assert_eq!(find_group_gid(sample, "repovec"), Some(996));
}

#[test]
fn error_display_and_path_are_operator_facing() {
    let violation = LiveLayoutViolation {
        path: Utf8PathBuf::from("/var/lib/repovec"),
        expected: "repovec (uid 1000)".to_owned(),
        actual: "uid 0".to_owned(),
    };
    let error = LiveLayoutError::IncorrectOwner { violation };

    assert_eq!(error.to_string(), "/var/lib/repovec must be owned by repovec (uid 1000)");
    assert_eq!(error.path(), Some("/var/lib/repovec"));
}

#[test]
fn group_lookup_error_display_names_the_database() {
    let error = super::group::GroupLookupError::NotFound;

    assert_eq!(error.to_string(), "the repovec system group does not exist");
}
