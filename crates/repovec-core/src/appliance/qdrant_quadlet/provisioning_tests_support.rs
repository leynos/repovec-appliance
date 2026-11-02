//! Packaging-asset fixtures and byte-offset helpers for provisioning tests.

macro_rules! include_packaging_asset {
    ($path:literal) => {
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../packaging/",
            $path
        ))
    };
}

pub(super) const PROVISIONING_SERVICE: &str =
    include_packaging_asset!("systemd/repovec-qdrant-api-key.service");
pub(super) const PROVISIONING_HELPER: &str =
    include_packaging_asset!("libexec/repovec-qdrant-api-key");
pub(super) const REPOVEC_SYSUSERS: &str = include_packaging_asset!("sysusers.d/repovec.conf");
pub(super) const QDRANT_API_KEY_PATH: &str = "/etc/repovec/qdrant-api-key";

/// Returns the byte offset of the first occurrence of `needle` in
/// `PROVISIONING_HELPER`, or `None` when the asset does not contain it.
fn find_helper_offset(needle: &str) -> Option<usize> { PROVISIONING_HELPER.find(needle) }

/// Asserts that `PROVISIONING_HELPER` contains `needle` and returns its offset.
///
/// This is a thin assertion wrapper over [`find_helper_offset`]: a missing
/// needle means the checked-in asset no longer matches the contract under
/// test, so the panic points at the caller.
#[track_caller]
pub(super) fn helper_offset(needle: &str) -> usize {
    let Some(offset) = find_helper_offset(needle) else {
        panic!("helper should contain expected text: {needle:?}");
    };
    offset
}

/// Returns the byte offset of `needle` at or after `offset`, or `None` when
/// either the boundary or the needle is invalid.
fn find_helper_offset_after(needle: &str, offset: usize) -> Option<usize> {
    let suffix = PROVISIONING_HELPER.get(offset..)?;
    suffix
        .find(needle)
        .map(|relative_offset| offset + relative_offset)
}

/// Asserts that `needle` occurs in `PROVISIONING_HELPER` after `offset`.
///
/// This is a thin assertion wrapper over [`find_helper_offset_after`] for
/// ordering assertions; the panic points at the caller.
#[track_caller]
pub(super) fn helper_offset_after(needle: &str, offset: usize) -> usize {
    let Some(found) = find_helper_offset_after(needle, offset) else {
        panic!(
            "helper should contain expected text after offset: needle={needle:?}, \
             offset={offset}, helper_len={}",
            PROVISIONING_HELPER.len()
        );
    };
    found
}

/// Returns the `start..end` slice of `PROVISIONING_HELPER`, or `None` when the
/// range is not a valid byte boundary.
fn find_helper_slice(start: usize, end: usize) -> Option<&'static str> {
    PROVISIONING_HELPER.get(start..end)
}

/// Asserts that `start..end` is a valid slice of `PROVISIONING_HELPER`.
///
/// This is a thin assertion wrapper over [`find_helper_slice`] for snapshot
/// assertions; the panic points at the caller.
#[track_caller]
pub(super) fn helper_slice(start: usize, end: usize) -> &'static str {
    let Some(slice) = find_helper_slice(start, end) else {
        panic!(
            "helper slice should use valid byte boundaries: start={start}, end={end}, \
             helper_len={}",
            PROVISIONING_HELPER.len()
        );
    };
    slice
}

/// Collects all byte offsets of `needle` in `PROVISIONING_HELPER`.
pub(super) fn all_helper_offsets(needle: &str) -> Vec<usize> {
    PROVISIONING_HELPER
        .match_indices(needle)
        .map(|(i, _)| i)
        .collect()
}
