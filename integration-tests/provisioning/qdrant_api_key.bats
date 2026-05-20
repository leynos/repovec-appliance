#!/usr/bin/env bats

load '../lib/test_helper.bash'

@test "creates repovec system user when absent" {
    remove_repovec_user

    run_helper

    run getent passwd "${REPOVEC_USER}"
    assert_success

    IFS=':' read -r _ _ _ _ _ home_dir login_shell <<<"${output}"
    assert_equal "${home_dir}" "/var/lib/repovec"
    assert_equal "${login_shell}" "/usr/sbin/nologin"
}

@test "creates key file with mode 0400 and ownership repovec:repovec" {
    rm -f "${KEY_FILE}"

    run_helper

    assert_regular_file_exists "${KEY_FILE}"
    run stat -c '%a %U:%G' "${KEY_FILE}"
    assert_success
    assert_output "400 repovec:repovec"

    run grep -Eq '^[[:xdigit:]]{64}$' "${KEY_FILE}"
    assert_success
}

@test "creates Podman secret repovec-qdrant-api-key" {
    remove_qdrant_secret

    run_helper

    run podman secret inspect --format '{{ .Spec.Name }}' "${SECRET_NAME}"
    assert_success
    assert_output "${SECRET_NAME}"
}

@test "preserves existing key file on re-run" {
    run_helper
    original_key="$(cat "${KEY_FILE}")"
    original_mtime="$(stat -c '%Y' "${KEY_FILE}")"

    run_helper

    refreshed_key="$(cat "${KEY_FILE}")"
    refreshed_mtime="$(stat -c '%Y' "${KEY_FILE}")"
    assert_equal "${refreshed_key}" "${original_key}"
    assert_equal "${refreshed_mtime}" "${original_mtime}"

    run podman secret inspect "${SECRET_NAME}"
    assert_success
}

@test "regenerates key file when absent and refreshes secret" {
    run_helper
    original_key="$(cat "${KEY_FILE}")"
    rm -f "${KEY_FILE}"

    run_helper

    assert_regular_file_exists "${KEY_FILE}"
    refreshed_key="$(cat "${KEY_FILE}")"
    refute_values_equal "${refreshed_key}" "${original_key}"

    run podman secret inspect "${SECRET_NAME}"
    assert_success
}

@test "creates /etc/repovec with mode 0750 and ownership root:repovec" {
    remove_repovec_config

    run_helper

    assert_directory_exists "${CONFIG_DIR}"
    run stat -c '%a %U:%G' "${CONFIG_DIR}"
    assert_success
    assert_output "750 root:repovec"
}
