#!/usr/bin/env bash
# Shared Bats helpers for provisioning tests that exercise host-level state.

load '../vendor/bats-support/load'
load '../vendor/bats-assert/load'

PROJECT_ROOT="$(cd "${BATS_TEST_DIRNAME}/../.." && pwd)"
HELPER_SCRIPT="${PROJECT_ROOT}/packaging/libexec/repovec-qdrant-api-key"
KEY_FILE="/etc/repovec/qdrant-api-key"
CONFIG_DIR="/etc/repovec"
SECRET_NAME="repovec-qdrant-api-key"
REPOVEC_USER="repovec"
REPOVEC_GROUP="repovec"

require_root() {
    if [ "$(id -u)" -ne 0 ]; then
        skip "provisioning integration tests require root privileges"
    fi
}

require_command() {
    local command_name="$1"

    if ! command -v "${command_name}" >/dev/null 2>&1; then
        skip "${command_name} is required for provisioning integration tests"
    fi
}

remove_repovec_user() {
    if getent passwd "${REPOVEC_USER}" >/dev/null 2>&1; then
        userdel -r "${REPOVEC_USER}" >/dev/null 2>&1 || userdel "${REPOVEC_USER}"
    fi
}

remove_qdrant_secret() {
    podman secret rm "${SECRET_NAME}" >/dev/null 2>&1 || true
}

remove_repovec_config() {
    rm -rf "${CONFIG_DIR}"
}

clean_provisioning_artifacts() {
    remove_qdrant_secret
    remove_repovec_config
    remove_repovec_user
}

setup() {
    require_root
    require_command getent
    require_command podman
    require_command stat
    require_command useradd
    require_command userdel
    clean_provisioning_artifacts
}

teardown() {
    clean_provisioning_artifacts
}

run_helper() {
    run "${HELPER_SCRIPT}"
    assert_success
}

assert_regular_file_exists() {
    local file_path="$1"

    if [ ! -f "${file_path}" ]; then
        fail "expected regular file to exist: ${file_path}"
    fi
}

assert_directory_exists() {
    local dir_path="$1"

    if [ ! -d "${dir_path}" ]; then
        fail "expected directory to exist: ${dir_path}"
    fi
}

refute_values_equal() {
    local actual="$1"
    local unexpected="$2"

    if [ "${actual}" = "${unexpected}" ]; then
        fail "expected values to differ"
    fi
}
