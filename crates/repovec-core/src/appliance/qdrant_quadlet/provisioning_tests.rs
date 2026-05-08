//! Unit tests covering the static Qdrant API-key provisioning assets.

use super::{
    QDRANT_API_KEY_ENVIRONMENT_VARIABLE, QDRANT_API_KEY_PATH, QDRANT_API_KEY_SECRET,
    QDRANT_API_KEY_SERVICE,
};

const PROVISIONING_SERVICE: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../packaging/systemd/repovec-qdrant-api-key.service",
));
const PROVISIONING_HELPER: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../packaging/libexec/repovec-qdrant-api-key",
));
const REPOVEC_SYSUSERS: &str =
    include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/../../packaging/sysusers.d/repovec.conf"));

#[test]
fn provisioning_service_runs_the_packaged_helper_once() {
    assert!(PROVISIONING_SERVICE.contains("[Service]"));
    assert!(PROVISIONING_SERVICE.contains("Type=oneshot"));
    assert!(PROVISIONING_SERVICE.contains("ExecStart=/usr/libexec/repovec/repovec-qdrant-api-key"));
}

#[test]
fn sysusers_asset_declares_the_repovec_system_user() {
    assert!(REPOVEC_SYSUSERS.contains("u repovec -"));
    assert!(REPOVEC_SYSUSERS.contains("/var/lib/repovec"));
    assert!(REPOVEC_SYSUSERS.contains("/usr/sbin/nologin"));
}

#[test]
fn provisioning_helper_uses_the_canonical_secret_contract() {
    assert!(PROVISIONING_HELPER.contains("KEY_FILE=\"${CONFIG_DIR}/qdrant-api-key\""));
    assert!(PROVISIONING_HELPER.contains(&format!("SECRET_NAME={QDRANT_API_KEY_SECRET}")));
    assert!(PROVISIONING_HELPER.contains("podman secret inspect \"${SECRET_NAME}\""));
    assert!(PROVISIONING_HELPER.contains("podman secret create \"${SECRET_NAME}\""));
    assert!(
        PROVISIONING_HELPER.contains(QDRANT_API_KEY_PATH.rsplit('/').next().unwrap_or_default())
    );
    assert!(PROVISIONING_SERVICE.contains(QDRANT_API_KEY_SERVICE.trim_end_matches(".service")));
    assert!(super::checked_in_qdrant_quadlet().contains(QDRANT_API_KEY_ENVIRONMENT_VARIABLE));
}

#[test]
fn provisioning_helper_preserves_existing_keys_and_locks_permissions() {
    assert!(PROVISIONING_HELPER.contains("if [ ! -e \"${KEY_FILE}\" ]; then"));
    assert!(
        PROVISIONING_HELPER.contains("chown \"${REPOVEC_USER}:${REPOVEC_GROUP}\" \"${KEY_FILE}\"")
    );
    assert!(PROVISIONING_HELPER.contains("chmod 0400 \"${KEY_FILE}\""));
    assert!(PROVISIONING_HELPER.contains("install -d -o root -g \"${REPOVEC_GROUP}\" -m 0750"));
}

#[test]
fn provisioning_helper_does_not_commit_or_echo_a_raw_key_literal() {
    assert!(!PROVISIONING_HELPER.contains("QDRANT__SERVICE__API_KEY="));
    assert!(!PROVISIONING_HELPER.contains("echo "));
    assert!(PROVISIONING_HELPER.contains("/dev/urandom"));
}
