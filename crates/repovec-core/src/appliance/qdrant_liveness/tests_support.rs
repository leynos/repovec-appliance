//! Temporary API-key files and shared constants for Qdrant liveness tests.

use std::{
    sync::atomic::{AtomicU64, Ordering},
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use camino::Utf8PathBuf;
use cap_std::{ambient_authority, fs_utf8::Dir};

pub(super) const TEST_QDRANT_ENDPOINT: &str = "http://127.0.0.1:6334";
pub(super) const TEST_API_KEY: &str = "repovec-test-qdrant-api-key";

static TEMP_COUNTER: AtomicU64 = AtomicU64::new(0);

pub(super) struct TempKeyFile {
    pub(super) path: Utf8PathBuf,
    dir_name: String,
}

impl TempKeyFile {
    pub(super) fn create(contents: &str) -> Result<Self, std::io::Error> {
        let temp_root = temp_root();
        let root = Dir::open_ambient_dir(&temp_root, ambient_authority())?;
        let dir_name = unique_temp_dir_name();
        root.create_dir(&dir_name)?;
        root.write(format!("{dir_name}/qdrant-api-key"), contents)?;

        Ok(Self {
            path: temp_root.join(&dir_name).join("qdrant-api-key"),
            dir_name,
        })
    }
}

impl Drop for TempKeyFile {
    fn drop(&mut self) {
        let temp_root = temp_root();
        let Ok(root) = Dir::open_ambient_dir(&temp_root, ambient_authority()) else {
            return;
        };
        if root.remove_dir_all(&self.dir_name).is_err() {}
    }
}

pub(super) fn temp_root() -> Utf8PathBuf { Utf8PathBuf::from("/tmp") }

pub(super) fn unique_temp_dir_name() -> String {
    let count = TEMP_COUNTER.fetch_add(1, Ordering::Relaxed);
    let elapsed = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or(Duration::ZERO);

    format!(
        "repovec-qdrant-liveness-{}-{}-{count}",
        std::process::id(),
        elapsed.as_nanos()
    )
}
