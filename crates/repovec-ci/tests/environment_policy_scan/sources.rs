//! The Rust sources the environment-policy scan reads.
//!
//! Which files are scanned is a separate concern from what the scan makes of
//! them, and keeping them apart is what holds each file inside the 400-line
//! limit.

use std::collections::VecDeque;

use camino::{Utf8Path, Utf8PathBuf};
use cap_std::{
    ambient_authority,
    fs_utf8::{Dir, DirEntry},
};

/// Directories holding Rust sources the policy governs.
pub const SOURCE_ROOTS: [&str; 1] = ["crates"];

/// The extension of a file this scan reads.
///
/// Named once because [`crate::tokens::includes_a_scanned_path`] decides
/// whether an `include!` target is already scanned, and it has to ask the same
/// question this module answers.
pub const SOURCE_EXTENSION: &str = "rs";

/// Return the repository root, from this crate's manifest directory.
pub fn repository_root() -> Utf8PathBuf {
    Utf8PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
}

/// Collect every `.rs` file under one root, breadth first.
///
/// Paths are reported relative to the repository root, because the
/// absolute one is built from this crate's manifest directory and reads
/// as `crates/repovec-ci/../../crates/...`, which is noise in a failure
/// a contributor has to act on.
///
/// A symlink is an error rather than something to step over.
/// [`crate::tokens::includes_a_scanned_path`] accepts an `include!` target
/// on containment alone, resting on every `.rs` file beneath a root being
/// read. This walk does not follow a symlink, so one skipped in silence
/// would break that premise without saying so: a target under a symlinked
/// directory would be contained, accepted and never scanned. Refusing it
/// here keeps the premise true, and fails closed if the repository ever
/// grows a link.
///
/// Mutation proof, recorded 2026-09-15: removing the refusal so a symlink is
/// stepped over fails both cases of
/// [`crate::workspace::a_symlink_under_a_source_root_is_an_error`] and
/// nothing else. It was found by probing the walk rather than by reading it,
/// after the containment rule was written on the premise it breaks.
///
/// The file case took three spellings to earn its line. Pointed outside the
/// tree, and then at an absolute path, it went on passing under the mutation,
/// because `cap_std` refuses to read through either kind of link and the
/// error it raises happens to name the link too. Only an in-tree relative
/// link is read when the refusal is gone. A fixture that survives its own
/// mutation discriminates nothing, however plausible it reads.
pub fn rust_sources(root: &Utf8Path, relative: &str) -> Result<Vec<(Utf8PathBuf, String)>, String> {
    let directory = Dir::open_ambient_dir(root, ambient_authority())
        .map_err(|error| format!("open {root}: {error}"))?;
    let mut walk = SourceWalk {
        pending: VecDeque::from([(directory, Utf8PathBuf::from(relative))]),
        sources: Vec::new(),
    };

    while let Some((current, prefix)) = walk.pending.pop_front() {
        walk.collect_directory(&current, &prefix)?;
    }

    Ok(walk.sources)
}

/// Breadth-first accumulator for the source walk.
struct SourceWalk {
    /// Directories still to visit, each with its repository-relative prefix.
    pending: VecDeque<(Dir, Utf8PathBuf)>,
    /// Collected source paths and contents, in visit order.
    sources: Vec<(Utf8PathBuf, String)>,
}

impl SourceWalk {
    /// Scans one directory level, queuing children and collecting sources.
    fn collect_directory(&mut self, current: &Dir, prefix: &Utf8Path) -> Result<(), String> {
        let entries = current
            .entries()
            .map_err(|error| format!("read {prefix}: {error}"))?;
        for candidate in entries {
            let entry = candidate.map_err(|error| format!("entry under {prefix}: {error}"))?;
            self.visit_entry(current, prefix, &entry)?;
        }
        Ok(())
    }

    /// Classifies one entry, rejecting links and reading Rust files.
    fn visit_entry(
        &mut self,
        current: &Dir,
        prefix: &Utf8Path,
        entry: &DirEntry,
    ) -> Result<(), String> {
        let name = entry
            .file_name()
            .map_err(|error| format!("name: {error}"))?;
        let path = prefix.join(&name);
        let kind = entry
            .file_type()
            .map_err(|error| format!("type of {path}: {error}"))?;

        if kind.is_symlink() {
            return Err(symlink_error(&path));
        }
        if kind.is_dir() {
            let child = current
                .open_dir(&name)
                .map_err(|error| format!("open {path}: {error}"))?;
            self.pending.push_back((child, path));
        } else if path.extension() == Some(SOURCE_EXTENSION) {
            let contents = current
                .read_to_string(&name)
                .map_err(|error| format!("read {path}: {error}"))?;
            self.sources.push((path, contents));
        }
        Ok(())
    }
}

/// Reports a symlink as a scan error rather than silently stepping over it.
fn symlink_error(path: &Utf8Path) -> String {
    format!(
        concat!(
            "{path} is a symlink; the scan does not follow one, so ",
            "a source reached through it would not be read"
        ),
        path = path
    )
}
