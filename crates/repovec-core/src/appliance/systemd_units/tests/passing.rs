//! Acceptance-path tests for the systemd unit validator.
//!
//! This module exercises valid configurations, comments, extended
//! `Environment=` settings, and other edge-but-valid variants to prove that
//! legitimate variation is accepted. It complements the failure-case mutation
//! tests by checking the `crate::appliance::systemd_units::validate_*`
//! functions against the checked-in unit accessors and the
//! `validate_systemd_units` / `validate_checked_in_systemd_units` helpers.

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
    let original_target = checked_in_repovec_target();
    assert!(
        original_target.contains("[Unit]\n"),
        "expected checked-in repovec target to contain '[Unit]\\n' but got: {original_target}",
    );
    let target =
        original_target.replace("[Unit]\n", "[Unit]\n; systemd accepts semicolon comments\n");
    assert_ne!(target, original_target, "replacement should mutate the target");

    validate_systemd_units(
        &target,
        checked_in_repovecd_service(),
        checked_in_repovec_mcpd_service(),
    )
    .expect("semicolon comments should be ignored");
}

#[test]
fn additional_service_environment_lines_are_accepted() {
    let original_repovecd = checked_in_repovecd_service();
    assert!(
        original_repovecd.contains("Environment=HOME=/var/lib/repovec\n"),
        "expected checked-in repovecd service to contain \
         'Environment=HOME=/var/lib/repovec\\n' but got: {original_repovecd}",
    );
    let repovecd = original_repovecd.replace(
        "Environment=HOME=/var/lib/repovec\n",
        "Environment=HOME=/var/lib/repovec\nEnvironment=SOME_OTHER_VAR=value\n",
    );
    assert_ne!(repovecd, original_repovecd, "replacement should mutate the repovecd service");

    let original_mcpd = checked_in_repovec_mcpd_service();
    assert!(
        original_mcpd.contains("Environment=HOME=/var/lib/repovec\n"),
        "expected checked-in repovec-mcpd service to contain \
         'Environment=HOME=/var/lib/repovec\\n' but got: {original_mcpd}",
    );
    let mcpd = original_mcpd.replace(
        "Environment=HOME=/var/lib/repovec\n",
        "Environment=HOME=/var/lib/repovec\nEnvironment=SOME_OTHER_VAR=value\n",
    );
    assert_ne!(mcpd, original_mcpd, "replacement should mutate the repovec-mcpd service");

    validate_systemd_units(checked_in_repovec_target(), &repovecd, &mcpd)
        .expect("additional service environment lines should be accepted");
}
