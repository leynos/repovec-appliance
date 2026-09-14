//! The Rust sources the environment-policy scan reads.
//!
//! Which files are scanned is a separate concern from what the scan makes of
//! them, and keeping them apart is what holds each file inside the 400-line
//! limit.

use std::collections::VecDeque;

use camino::{Utf8Path, Utf8PathBuf};
use cap_std::{ambient_authority, fs_utf8::Dir};

/// Directories holding Rust sources the policy governs.
pub const SOURCE_ROOTS: [&str; 1] = ["crates"];

/// Return the repository root, from this crate's manifest directory.
pub fn repository_root() -> Utf8PathBuf {
    Utf8PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("..").join("..")
}

/// Collect every `.rs` file under one root, depth first.
///
/// Paths are reported relative to the repository root, because the
/// absolute one is built from this crate's manifest directory and reads
/// as `crates/repovec-ci/../../crates/...`, which is noise in a failure
/// a contributor has to act on.
pub fn rust_sources(root: &Utf8Path, relative: &str) -> Result<Vec<(Utf8PathBuf, String)>, String> {
    let directory = Dir::open_ambient_dir(root, ambient_authority())
        .map_err(|error| format!("open {root}: {error}"))?;
    let mut pending = VecDeque::from([(directory, Utf8PathBuf::from(relative))]);
    let mut sources = Vec::new();

    while let Some((current, prefix)) = pending.pop_front() {
        let entries = current.entries().map_err(|error| format!("read {prefix}: {error}"))?;
        for candidate in entries {
            let entry = candidate.map_err(|error| format!("entry under {prefix}: {error}"))?;
            let name = entry.file_name().map_err(|error| format!("name: {error}"))?;
            let path = prefix.join(&name);
            let file_type =
                entry.file_type().map_err(|error| format!("type of {path}: {error}"))?;
            if file_type.is_dir() {
                let child =
                    current.open_dir(&name).map_err(|error| format!("open {path}: {error}"))?;
                pending.push_back((child, path));
            } else if path.extension() == Some("rs") {
                let contents = current
                    .read_to_string(&name)
                    .map_err(|error| format!("read {path}: {error}"))?;
                sources.push((path, contents));
            }
        }
    }
    Ok(sources)
}
