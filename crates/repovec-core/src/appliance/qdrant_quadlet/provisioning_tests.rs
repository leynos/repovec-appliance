//! Unit tests covering the static Qdrant API-key provisioning assets.
//!
//! These tests validate the static contract between the `mod.rs` constants
//! `QDRANT_API_KEY_SECRET`, `QDRANT_API_KEY_SERVICE`, and
//! `QDRANT_API_KEY_ENVIRONMENT_VARIABLE`, and the packaging assets under
//! `packaging/`.

use super::{QDRANT_API_KEY_ENVIRONMENT_VARIABLE, QDRANT_API_KEY_SECRET, QDRANT_API_KEY_SERVICE};

macro_rules! include_packaging_asset {
    ($path:literal) => {
        include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/../../packaging/", $path))
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
    assert!(PROVISIONING_HELPER.contains("podman secret rm \"${SECRET_NAME}\""));
    assert!(PROVISIONING_HELPER.contains("grep -qi 'in use' \"${rm_error}\""));
    assert!(PROVISIONING_HELPER.contains("log \"podman secret removal failed: $(cat"));
    assert!(PROVISIONING_HELPER.contains("exit 1"));
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
fn provisioning_helper_does_not_commit_or_echo_a_raw_key_literal() {
    assert!(!PROVISIONING_HELPER.contains("QDRANT__SERVICE__API_KEY="));
    assert!(!PROVISIONING_HELPER.contains("echo \"${KEY_FILE}\""));
    assert!(!PROVISIONING_HELPER.contains("echo \"${tmp_file}\""));
    assert!(PROVISIONING_HELPER.contains("/dev/urandom"));
}
