//! Tests for service identity and runtime-directory invariants.

use super::{
    SystemdUnitError, checked_in_repovec_mcpd_service, checked_in_repovec_target,
    checked_in_repovecd_service, validate_systemd_units,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ServiceFile {
    Repovecd,
    Mcpd,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct ServiceCase {
    file: ServiceFile,
    from: &'static str,
    to: &'static str,
    unit: &'static str,
    key: &'static str,
    expected: &'static str,
    actual: &'static str,
}

#[test]
fn service_identity_and_runtime_settings_are_required() {
    for service_case in missing_repovecd_cases().into_iter().chain(wrong_mcpd_cases()) {
        let err = validate_mutated_service(service_case);

        assert_eq!(
            err,
            setting(
                service_case.unit,
                service_case.key,
                service_case.expected,
                service_case.actual
            ),
        );
        assert_eq!(
            err.to_string(),
            format!(
                "{} must set {}={} in [Service]: {}",
                service_case.unit, service_case.key, service_case.expected, service_case.actual,
            ),
        );
    }
}

fn validate_mutated_service(service_case: ServiceCase) -> SystemdUnitError {
    let target = checked_in_repovec_target();
    let mut repovecd = checked_in_repovecd_service().to_owned();
    let mut mcpd = checked_in_repovec_mcpd_service().to_owned();
    let service = match service_case.file {
        ServiceFile::Repovecd => &mut repovecd,
        ServiceFile::Mcpd => &mut mcpd,
    };
    *service = service.replace(service_case.from, service_case.to);

    validate_systemd_units(target, &repovecd, &mcpd)
        .expect_err("expected missing service setting validation to fail")
}

fn missing_repovecd_cases() -> [ServiceCase; 4] {
    [
        ServiceCase {
            file: ServiceFile::Repovecd,
            from: "User=repovec\n",
            to: "",
            unit: "repovecd.service",
            key: "User",
            expected: "repovec",
            actual: "",
        },
        ServiceCase {
            file: ServiceFile::Repovecd,
            from: "Group=repovec\n",
            to: "",
            unit: "repovecd.service",
            key: "Group",
            expected: "repovec",
            actual: "",
        },
        ServiceCase {
            file: ServiceFile::Repovecd,
            from: "WorkingDirectory=/var/lib/repovec\n",
            to: "",
            unit: "repovecd.service",
            key: "WorkingDirectory",
            expected: "/var/lib/repovec",
            actual: "",
        },
        ServiceCase {
            file: ServiceFile::Repovecd,
            from: "Environment=HOME=/var/lib/repovec\n",
            to: "",
            unit: "repovecd.service",
            key: "Environment",
            expected: "HOME=/var/lib/repovec",
            actual: "",
        },
    ]
}

fn wrong_mcpd_cases() -> [ServiceCase; 4] {
    [
        ServiceCase {
            file: ServiceFile::Mcpd,
            from: "User=repovec\n",
            to: "User=root\n",
            unit: "repovec-mcpd.service",
            key: "User",
            expected: "repovec",
            actual: "root",
        },
        ServiceCase {
            file: ServiceFile::Mcpd,
            from: "Group=repovec\n",
            to: "Group=root\n",
            unit: "repovec-mcpd.service",
            key: "Group",
            expected: "repovec",
            actual: "root",
        },
        ServiceCase {
            file: ServiceFile::Mcpd,
            from: "WorkingDirectory=/var/lib/repovec\n",
            to: "WorkingDirectory=/tmp\n",
            unit: "repovec-mcpd.service",
            key: "WorkingDirectory",
            expected: "/var/lib/repovec",
            actual: "/tmp",
        },
        ServiceCase {
            file: ServiceFile::Mcpd,
            from: "Environment=HOME=/var/lib/repovec\n",
            to: "Environment=HOME=/tmp\n",
            unit: "repovec-mcpd.service",
            key: "Environment",
            expected: "HOME=/var/lib/repovec",
            actual: "HOME=/tmp",
        },
    ]
}

fn setting(
    unit: &'static str,
    key: &'static str,
    expected: &'static str,
    actual: &str,
) -> SystemdUnitError {
    SystemdUnitError::MissingSetting {
        unit,
        section: "Service",
        key,
        expected,
        actual: actual.to_owned(),
    }
}
