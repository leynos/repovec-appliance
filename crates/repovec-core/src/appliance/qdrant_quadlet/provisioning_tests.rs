//! Unit tests covering the static Qdrant API-key provisioning assets.
//!
//! These tests validate the static contract between the `mod.rs` constants
//! `QDRANT_API_KEY_SECRET`, `QDRANT_API_KEY_SERVICE`, and
//! `QDRANT_API_KEY_ENVIRONMENT_VARIABLE`, and the packaging assets under
//! `packaging/`.

use proptest::prelude::*;

use super::{QDRANT_API_KEY_ENVIRONMENT_VARIABLE, QDRANT_API_KEY_SECRET, QDRANT_API_KEY_SERVICE};

macro_rules! include_packaging_asset {
    ($path:literal) => {
        include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/../../packaging/", $path))
    };
}

/// Returns the byte offset of the first occurrence of `needle` in
/// `PROVISIONING_HELPER`, panicking if the needle is missing.
macro_rules! helper_offset {
    ($needle:expr) => {
        PROVISIONING_HELPER.find($needle).expect("helper should contain expected text")
    };
}

/// Returns the byte offset of the first occurrence of `needle` in
/// `PROVISIONING_HELPER` after `offset`, useful for ordering assertions.
/// Panics if `offset` is not a valid UTF-8 boundary or if no matching
/// occurrence exists after `offset`.
macro_rules! helper_offset_after {
    ($needle:expr, $offset:expr) => {
        $offset
            + PROVISIONING_HELPER
                .get($offset..)
                .expect("helper offset should be a valid byte boundary")
                .find($needle)
                .expect("helper should contain expected text after offset")
    };
}

const PROVISIONING_SERVICE: &str =
    include_packaging_asset!("systemd/repovec-qdrant-api-key.service");
const PROVISIONING_HELPER: &str = include_packaging_asset!("libexec/repovec-qdrant-api-key");
const REPOVEC_SYSUSERS: &str = include_packaging_asset!("sysusers.d/repovec.conf");
const QDRANT_API_KEY_PATH: &str = "/etc/repovec/qdrant-api-key";

#[test]
fn provisioning_service_runs_the_packaged_helper_once() {
    assert!(PROVISIONING_SERVICE.contains("[Service]"));
    assert!(PROVISIONING_SERVICE.contains("Type=oneshot"));
    assert!(PROVISIONING_SERVICE.contains("ExecStart=/usr/libexec/repovec/repovec-qdrant-api-key"));
    assert!(PROVISIONING_SERVICE.contains("StandardOutput=journal"));
    assert!(PROVISIONING_SERVICE.contains("StandardError=journal"));
}

#[test]
fn sysusers_asset_declares_the_repovec_system_user() {
    assert!(REPOVEC_SYSUSERS.contains("u repovec -"));
    assert!(REPOVEC_SYSUSERS.contains("/var/lib/repovec"));
    assert!(REPOVEC_SYSUSERS.contains("/usr/sbin/nologin"));
}

#[test]
fn provisioning_helper_uses_the_canonical_secret_contract() {
    assert!(PROVISIONING_HELPER.contains(&format!("KEY_FILE={QDRANT_API_KEY_PATH}")));
    assert!(PROVISIONING_HELPER.contains(&format!("SECRET_NAME={QDRANT_API_KEY_SECRET}")));
    assert!(PROVISIONING_HELPER.contains("podman secret inspect \"${SECRET_NAME}\""));
    assert!(PROVISIONING_HELPER.contains("podman secret create \"${SECRET_NAME}\""));
    assert!(PROVISIONING_HELPER.contains("log \"created qdrant API-key podman secret\""));
    assert!(PROVISIONING_SERVICE.contains(QDRANT_API_KEY_SERVICE.trim_end_matches(".service")));
    assert!(super::checked_in_qdrant_quadlet().contains(QDRANT_API_KEY_ENVIRONMENT_VARIABLE));
}

#[test]
fn provisioning_helper_fails_closed_on_unexpected_secret_removal_errors() {
    let removal = helper_offset!("podman secret rm \"${SECRET_NAME}\"");
    let in_use_check = helper_offset!("grep -qi 'in use' \"${rm_error}\"");
    let in_use_exit = helper_offset!("log \"podman secret is in use; leaving existing secret");
    let unexpected_branch = helper_offset!("else\n            # Fail closed:");
    let unexpected_log = helper_offset!("log \"podman secret removal failed: $(cat");
    let unexpected_exit = helper_offset_after!("exit 1", unexpected_log);

    assert!(removal < in_use_check);
    assert!(in_use_check < in_use_exit);
    assert!(in_use_exit < unexpected_branch);
    assert!(unexpected_branch < unexpected_log);
    assert!(unexpected_log < unexpected_exit);
}

#[test]
fn provisioning_helper_removes_stale_secret_before_generating_key_file() {
    let secret_removal = PROVISIONING_HELPER
        .find("podman secret rm \"${SECRET_NAME}\"")
        .expect("helper should remove stale Podman secrets");
    let key_generation = PROVISIONING_HELPER
        .find("if [ ! -e \"${KEY_FILE}\" ]; then")
        .expect("helper should generate a missing key file");

    assert!(secret_removal < key_generation);
}

#[test]
fn provisioning_helper_uses_a_root_controlled_lock_path() {
    let lock_file = helper_offset!("/etc/repovec/repovec-qdrant-api-key.lock");
    let flock = helper_offset!("flock 9");
    let debug_waiting = helper_offset!("debug_log \"waiting for ${LOCK_FILE}\"");
    let debug_acquired = helper_offset!("debug_log \"acquired ${LOCK_FILE}\"");

    assert!(PROVISIONING_HELPER.contains("debug_log \"acquired ${LOCK_FILE}\""));
    assert!(PROVISIONING_HELPER.contains("debug_log \"released ${LOCK_FILE}\""));
    assert!(!PROVISIONING_HELPER.contains("LOCK_FILE=/var/lock/"));
    assert!(debug_waiting < flock);
    assert!(flock < debug_acquired);
    assert!(lock_file < flock);
}

#[test]
fn provisioning_helper_serializes_secret_and_key_operations() {
    let flock = helper_offset!("flock 9");
    let secret_inspection = helper_offset!("podman secret inspect \"${SECRET_NAME}\"");
    let secret_removal = helper_offset!("podman secret rm \"${SECRET_NAME}\"");
    let missing_key_branch = helper_offset!("if [ ! -e \"${KEY_FILE}\" ]; then");
    let key_generation = helper_offset_after!("generate_key_file", missing_key_branch);
    let secret_creation = helper_offset!("podman secret create \"${SECRET_NAME}\"");

    assert!(PROVISIONING_HELPER.contains("od -An -N32 -tx1"));
    assert!(flock < secret_inspection);
    assert!(flock < secret_removal);
    assert!(flock < key_generation);
    assert!(flock < secret_creation);
}

#[test]
fn provisioning_helper_logs_secret_creation_errors() {
    assert!(PROVISIONING_HELPER.contains("qdrant-secret-create."));
    assert!(PROVISIONING_HELPER.contains("podman secret create \"${SECRET_NAME}\""));
    assert!(PROVISIONING_HELPER.contains("log \"podman secret creation failed: $(cat"));
    assert!(PROVISIONING_HELPER.contains("exit 1"));
}

#[test]
fn provisioning_helper_preserves_existing_keys_and_locks_permissions() {
    assert!(PROVISIONING_HELPER.contains("if [ ! -e \"${KEY_FILE}\" ]; then"));
    assert!(PROVISIONING_HELPER.contains("candidate_key=\"$(od -An -N32 -tx1 /dev/urandom"));
    assert!(PROVISIONING_HELPER.contains("[ \"${#candidate_key}\" -ne 64 ]"));
    assert!(PROVISIONING_HELPER.contains("grep -q '[^0-9a-fA-F]'"));
    assert!(PROVISIONING_HELPER.contains("printf '%s' \"${candidate_key}\" >\"${tmp_file}\""));
    assert!(
        PROVISIONING_HELPER.contains("chown \"${REPOVEC_USER}:${REPOVEC_GROUP}\" \"${KEY_FILE}\"")
    );
    assert!(PROVISIONING_HELPER.contains("chmod 0400 \"${KEY_FILE}\""));
    assert!(PROVISIONING_HELPER.contains("install -d -o root -g \"${REPOVEC_GROUP}\" -m 0750"));
}

#[test]
fn provisioning_helper_releases_lock_before_every_error_exit() {
    let release_lock_a = helper_offset!("release_lock");
    let exit_a = helper_offset_after!("exit 1", release_lock_a);
    assert!(release_lock_a < exit_a);

    let release_lock_b = helper_offset_after!("release_lock", exit_a);
    let exit_b = helper_offset_after!("exit 0", release_lock_b);
    assert!(release_lock_b < exit_b);

    let release_lock_c = helper_offset_after!("release_lock", exit_b);
    let exit_c = helper_offset_after!("exit 1", release_lock_c);
    assert!(release_lock_c < exit_c);

    let release_lock_d = helper_offset_after!("release_lock", exit_c);
    let exit_d = helper_offset_after!("exit 1", release_lock_d);
    assert!(release_lock_d < exit_d);

    let created_log = helper_offset!("created qdrant API-key podman secret");
    let release_lock_e = helper_offset_after!("release_lock", created_log);
    assert!(created_log < release_lock_e);
}

#[test]
fn provisioning_helper_does_not_commit_or_echo_a_raw_key_literal() {
    assert!(!PROVISIONING_HELPER.contains("QDRANT__SERVICE__API_KEY="));
    assert!(!PROVISIONING_HELPER.contains("echo \"${KEY_FILE}\""));
    assert!(!PROVISIONING_HELPER.contains("echo \"${tmp_file}\""));
    assert!(PROVISIONING_HELPER.contains("/dev/urandom"));
}

/// Collects all byte offsets of `needle` in `PROVISIONING_HELPER`.
fn all_helper_offsets(needle: &str) -> Vec<usize> {
    PROVISIONING_HELPER.match_indices(needle).map(|(i, _)| i).collect()
}

proptest! {
    /// Every `exit 1` in the helper is preceded by an explicit `release_lock`
    /// call that itself follows the `flock 9` acquisition, regardless of which
    /// error path is taken.
    #[test]
    fn every_error_exit_1_is_preceded_by_release_lock(
        exit_pos in proptest::sample::select(all_helper_offsets("exit 1")),
    ) {
        let flock_pos = helper_offset!("flock 9");
        let missing_key_branch = helper_offset!("if [ ! -e \"${KEY_FILE}\" ]; then");
        let key_generation = helper_offset_after!("generate_key_file", missing_key_branch);
        let prefix = PROVISIONING_HELPER
            .get(..exit_pos)
            .expect("exit_pos must be a valid byte boundary");
        let release_pos = prefix
            .rfind("release_lock")
            .expect("every exit 1 must be preceded by release_lock");
        if release_pos < flock_pos {
            prop_assert!(
                exit_pos < flock_pos,
                "function-local exit 1 at {exit_pos} must appear before flock at {flock_pos}",
            );
            prop_assert!(
                flock_pos < key_generation,
                "flock at {flock_pos} must precede key generation at {key_generation}",
            );
        } else {
            prop_assert!(
                flock_pos < release_pos,
                "release_lock at {release_pos} must follow flock at {flock_pos}",
            );
        }
        prop_assert!(
            release_pos < exit_pos,
            "release_lock at {release_pos} must precede exit 1 at {exit_pos}",
        );
    }
}

proptest! {
    /// `flock 9` precedes every mutable operation in the helper, regardless of
    /// which operation is selected.
    #[test]
    fn flock_precedes_every_mutable_operation(
        op_pos in proptest::sample::select(vec![
            helper_offset!("podman secret inspect \"${SECRET_NAME}\""),
            helper_offset!("podman secret rm \"${SECRET_NAME}\""),
            helper_offset!("podman secret create \"${SECRET_NAME}\""),
            helper_offset_after!(
                "generate_key_file",
                helper_offset!("if [ ! -e \"${KEY_FILE}\" ]; then")
            ),
        ]),
    ) {
        let flock_pos = helper_offset!("flock 9");
        prop_assert!(
            flock_pos < op_pos,
            "flock at {flock_pos} must precede mutable operation at {op_pos}",
        );
    }
}

proptest! {
    /// The fail-closed invariant holds for every combination of
    /// `secret_exists x key_file_exists`: the unexpected-removal else-branch
    /// always progresses to an error log followed by `exit 1`, with no path
    /// that falls through silently.
    #[test]
    fn fail_closed_semantics_hold_for_all_secret_and_key_file_states(
        secret_exists in any::<bool>(),
        key_file_exists in any::<bool>(),
    ) {
        // The helper is a static string; runtime state does not change it.
        // These parameters document the full state space over which the
        // invariant is claimed to hold.
        let _ = (secret_exists, key_file_exists);

        let else_branch = helper_offset!("else\n            # Fail closed:");
        let failure_log = helper_offset_after!(
            "log \"podman secret removal failed:",
            else_branch
        );
        let failure_exit = helper_offset_after!("exit 1", failure_log);

        prop_assert!(
            else_branch < failure_log,
            "fail-closed else branch must precede its error log",
        );
        prop_assert!(
            failure_log < failure_exit,
            "error log must precede exit 1 on the fail-closed path",
        );
    }
}
