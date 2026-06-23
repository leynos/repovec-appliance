//! Grepai template hardening coverage for systemd unit validator tests.

use rstest::rstest;

use super::{
    super::{
        GREPAI_BOOLEAN_HARDENING_DIRECTIVES, GREPAI_HARDENING_DIRECTIVES,
        GREPAI_RESTRICT_ADDRESS_FAMILIES, REPOVEC_GREPAI_TEMPLATE_UNIT, SystemdUnitError,
    },
    unit_set::{UnitFile, UnitSet, checked_in_unit_set},
};

#[rstest]
#[case::no_new_privileges("NoNewPrivileges", "yes")]
#[case::private_tmp("PrivateTmp", "yes")]
#[case::protect_system("ProtectSystem", "full")]
#[case::protect_home("ProtectHome", "read-only")]
#[case::private_devices("PrivateDevices", "yes")]
#[case::device_policy("DevicePolicy", "closed")]
#[case::lock_personality("LockPersonality", "yes")]
#[case::protect_clock("ProtectClock", "yes")]
#[case::protect_control_groups("ProtectControlGroups", "yes")]
#[case::protect_hostname("ProtectHostname", "yes")]
#[case::protect_kernel_logs("ProtectKernelLogs", "yes")]
#[case::protect_kernel_modules("ProtectKernelModules", "yes")]
#[case::protect_kernel_tunables("ProtectKernelTunables", "yes")]
#[case::protect_proc("ProtectProc", "invisible")]
#[case::proc_subset("ProcSubset", "pid")]
#[case::restrict_namespaces("RestrictNamespaces", "yes")]
#[case::restrict_realtime("RestrictRealtime", "yes")]
#[case::restrict_suid_sgid("RestrictSUIDSGID", "yes")]
#[case::restrict_address_families("RestrictAddressFamilies", GREPAI_RESTRICT_ADDRESS_FAMILIES)]
fn grepai_template_requires_hardening_directives(
    checked_in_unit_set: UnitSet,
    #[case] key: &'static str,
    #[case] expected: &'static str,
) {
    let mut units = checked_in_unit_set;
    let directive = format!("{key}={expected}\n");
    units.remove_line(UnitFile::GrepaiTemplate, &directive);

    assert_eq!(
        units.validate(),
        Err(SystemdUnitError::IncorrectServiceDirective {
            unit: REPOVEC_GREPAI_TEMPLATE_UNIT,
            key,
            expected,
            actual: String::new(),
        }),
    );
}

#[test]
fn grepai_hardening_cases_cover_the_validator_contract() {
    assert_eq!(
        GREPAI_HARDENING_DIRECTIVES.len() + GREPAI_BOOLEAN_HARDENING_DIRECTIVES.len() + 1,
        19,
    );
}
