//! Atomic filesystem writes for encrypted credential material.

use std::{
    io::{ErrorKind, Write},
    sync::atomic::{AtomicU64, Ordering},
};

#[cfg(unix)]
use cap_std::fs_utf8::OpenOptionsExt;
use cap_std::fs_utf8::{Dir, OpenOptions};

const CREATE_RETRIES: u8 = 8;

static NEXT_TEMP_FILE_ID: AtomicU64 = AtomicU64::new(0);

pub(super) fn write_atomically(root: &Dir, filename: &str, contents: &[u8]) -> std::io::Result<()> {
    let (temp_name, mut temp_file) = create_temp_file(root, filename)?;
    temp_file.write_all(contents)?;
    temp_file.flush()?;
    temp_file.sync_all()?;
    drop(temp_file);
    root.rename(&temp_name, root, filename)?;
    root.open(".")?.sync_all()
}

fn create_temp_file(
    root: &Dir,
    filename: &str,
) -> std::io::Result<(String, cap_std::fs_utf8::File)> {
    for _ in 0..CREATE_RETRIES {
        let temp_name = next_temp_name(filename);
        let mut options = OpenOptions::new();
        options.create_new(true).write(true);
        #[cfg(unix)]
        options.mode(0o600);
        match root.open_with(&temp_name, &options) {
            Ok(file) => return Ok((temp_name, file)),
            Err(error) if error.kind() == ErrorKind::AlreadyExists => {}
            Err(error) => return Err(error),
        }
    }
    Err(std::io::Error::new(
        ErrorKind::AlreadyExists,
        "could not create a unique atomic-write temporary file",
    ))
}

fn next_temp_name(filename: &str) -> String {
    let id = NEXT_TEMP_FILE_ID.fetch_add(1, Ordering::Relaxed);
    format!(".{filename}.{}.{id}.tmp", std::process::id())
}
