//! Passing cases for systemd unit validator tests.

use super::super::{
    checked_in_repovec_mcpd_service, checked_in_repovec_target, checked_in_repovecd_service,
    validate_checked_in_systemd_units, validate_systemd_units,
};

#[test]
fn checked_in_systemd_units_remain_valid() {
    validate_checked_in_systemd_units()
        .expect("the checked-in repovec systemd unit set should remain valid");
}

#[test]
fn semicolon_comments_are_ignored() {
    assert!(checked_in_repovec_target().contains("[Unit]\n"));
    let target = checked_in_repovec_target()
        .replace("[Unit]\n", "[Unit]\n; systemd accepts semicolon comments\n");

    validate_systemd_units(
        &target,
        checked_in_repovecd_service(),
        checked_in_repovec_mcpd_service(),
    )
    .expect("semicolon comments should be ignored");
}

#[test]
fn additional_service_environment_lines_are_accepted() {
    assert!(checked_in_repovecd_service().contains("Environment=HOME=/var/lib/repovec\n"));
    let repovecd = checked_in_repovecd_service().replace(
        "Environment=HOME=/var/lib/repovec\n",
        "Environment=HOME=/var/lib/repovec\nEnvironment=SOME_OTHER_VAR=value\n",
    );
    assert!(checked_in_repovec_mcpd_service().contains("Environment=HOME=/var/lib/repovec\n"));
    let mcpd = checked_in_repovec_mcpd_service().replace(
        "Environment=HOME=/var/lib/repovec\n",
        "Environment=HOME=/var/lib/repovec\nEnvironment=SOME_OTHER_VAR=value\n",
    );

    validate_systemd_units(checked_in_repovec_target(), &repovecd, &mcpd)
        .expect("additional service environment lines should be accepted");
}
