//! Contract tests for `validate_systemd_units`: deterministic mutations of the
//! shipped systemd units plus literal diagnostic assertions.

#[path = "tests/diagnostics.rs"]
mod diagnostics;
#[path = "tests/unit_set.rs"]
mod unit_set;

use diagnostics::expected_diagnostic;
use rstest::{fixture, rstest};
use unit_set::{UnitFile, UnitSet};

use super::{
    SystemdUnitError, checked_in_repovec_mcpd_service, checked_in_repovec_target,
    checked_in_repovecd_service, validate_checked_in_systemd_units, validate_systemd_units,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum ValidationScenario {
    InvalidLine,
    PropertyBeforeSection,
    MissingTargetUnitSection,
    MissingTargetInstallSection,
    MissingTargetWantedBy,
    MissingTargetWantsQdrant,
    TargetUsesQdrantContainer,
    MissingTargetWantsRepovecd,
    MissingTargetWantsMcpd,
    MissingTargetWantsCloudflared,
    MissingRepovecdRequiresQdrant,
    MissingRepovecdAfterQdrant,
    RepovecdUsesQdrantContainerService,
    WrongRepovecdExecStart,
    RepovecdWrongUser,
    RepovecdMissingGroup,
    MissingMcpdRequiresQdrant,
    MissingMcpdRequiresRepovecd,
    MissingMcpdAfterQdrant,
    MissingMcpdAfterRepovecd,
    WrongMcpdExecStart,
    McpdWrongWorkingDirectory,
    McpdMissingEnvironment,
}

impl ValidationScenario {
    fn mutate(self, mut units: UnitSet) -> UnitSet {
        match self {
            Self::InvalidLine
            | Self::MissingTargetUnitSection
            | Self::MissingTargetInstallSection
            | Self::MissingTargetWantedBy
            | Self::MissingTargetWantsQdrant
            | Self::TargetUsesQdrantContainer
            | Self::MissingTargetWantsRepovecd
            | Self::MissingTargetWantsMcpd
            | Self::MissingTargetWantsCloudflared => self.mutate_target(&mut units),
            Self::PropertyBeforeSection
            | Self::MissingRepovecdRequiresQdrant
            | Self::MissingRepovecdAfterQdrant
            | Self::RepovecdUsesQdrantContainerService
            | Self::WrongRepovecdExecStart
            | Self::RepovecdWrongUser
            | Self::RepovecdMissingGroup => self.mutate_repovecd(&mut units),
            Self::MissingMcpdRequiresQdrant
            | Self::MissingMcpdRequiresRepovecd
            | Self::MissingMcpdAfterQdrant
            | Self::MissingMcpdAfterRepovecd
            | Self::WrongMcpdExecStart
            | Self::McpdWrongWorkingDirectory
            | Self::McpdMissingEnvironment => self.mutate_mcpd(&mut units),
        }

        units
    }

    fn mutate_target(self, units: &mut UnitSet) {
        match self {
            Self::InvalidLine => units.replace_file(UnitFile::Target, "[Unit]\nnot valid\n"),
            Self::MissingTargetUnitSection => {
                units.replace_file(UnitFile::Target, "[Install]\nWantedBy=multi-user.target\n");
            }
            Self::MissingTargetInstallSection => {
                units.remove_line(UnitFile::Target, "[Install]\n");
                units.remove_line(UnitFile::Target, "WantedBy=multi-user.target\n");
            }
            Self::MissingTargetWantedBy => {
                units.remove_line(UnitFile::Target, "WantedBy=multi-user.target\n");
            }
            Self::MissingTargetWantsQdrant => {
                units.remove_token(UnitFile::Target, "Wants=", "qdrant.service");
            }
            Self::TargetUsesQdrantContainer => {
                units.replace_token(UnitFile::Target, "qdrant.service", "qdrant.container");
            }
            Self::MissingTargetWantsRepovecd => {
                units.remove_token(UnitFile::Target, "Wants=", "repovecd.service");
            }
            Self::MissingTargetWantsMcpd => {
                units.remove_token(UnitFile::Target, "Wants=", "repovec-mcpd.service");
            }
            Self::MissingTargetWantsCloudflared => {
                units.remove_token(UnitFile::Target, "Wants=", "cloudflared.service");
            }
            _ => panic!("target mutation called for non-target scenario"),
        }
    }

    fn mutate_repovecd(self, units: &mut UnitSet) {
        match self {
            Self::PropertyBeforeSection => {
                units.replace_file(UnitFile::Repovecd, "Requires=qdrant.service\n[Unit]\n");
            }
            Self::MissingRepovecdRequiresQdrant => {
                units.remove_line(UnitFile::Repovecd, "Requires=qdrant.service\n");
            }
            Self::MissingRepovecdAfterQdrant => {
                units.remove_line(UnitFile::Repovecd, "After=qdrant.service\n");
            }
            Self::RepovecdUsesQdrantContainerService => units.replace_token(
                UnitFile::Repovecd,
                "qdrant.service",
                "qdrant.container.service",
            ),
            Self::WrongRepovecdExecStart => {
                units.replace_token(UnitFile::Repovecd, "/usr/bin/repovecd", "/usr/bin/otherd");
            }
            Self::RepovecdWrongUser => {
                units.replace_token(UnitFile::Repovecd, "User=repovec", "User=root");
            }
            Self::RepovecdMissingGroup => {
                units.remove_line(UnitFile::Repovecd, "Group=repovec\n");
            }
            _ => panic!("repovecd mutation called for non-repovecd scenario"),
        }
    }

    fn mutate_mcpd(self, units: &mut UnitSet) {
        match self {
            Self::MissingMcpdRequiresQdrant => {
                units.remove_token(UnitFile::Mcpd, "Requires=", "qdrant.service");
            }
            Self::MissingMcpdRequiresRepovecd => {
                units.remove_token(UnitFile::Mcpd, "Requires=", "repovecd.service");
            }
            Self::MissingMcpdAfterQdrant => {
                units.remove_token(UnitFile::Mcpd, "After=", "qdrant.service");
            }
            Self::MissingMcpdAfterRepovecd => {
                units.remove_token(UnitFile::Mcpd, "After=", "repovecd.service");
            }
            Self::WrongMcpdExecStart => {
                units.replace_token(UnitFile::Mcpd, "/usr/bin/repovec-mcpd", "/usr/bin/other-mcpd");
            }
            Self::McpdWrongWorkingDirectory => units.replace_token(
                UnitFile::Mcpd,
                "WorkingDirectory=/var/lib/repovec",
                "WorkingDirectory=/tmp",
            ),
            Self::McpdMissingEnvironment => {
                units.remove_line(UnitFile::Mcpd, "Environment=HOME=/var/lib/repovec\n");
            }
            _ => panic!("repovec-mcpd mutation called for non-mcpd scenario"),
        }
    }

    fn expected_error(self) -> SystemdUnitError {
        match self {
            Self::InvalidLine
            | Self::PropertyBeforeSection
            | Self::MissingTargetUnitSection
            | Self::MissingTargetInstallSection
            | Self::MissingTargetWantedBy
            | Self::MissingTargetWantsQdrant
            | Self::TargetUsesQdrantContainer
            | Self::MissingTargetWantsRepovecd
            | Self::MissingTargetWantsMcpd
            | Self::MissingTargetWantsCloudflared => self.expected_target_error(),
            Self::MissingRepovecdRequiresQdrant
            | Self::MissingRepovecdAfterQdrant
            | Self::RepovecdUsesQdrantContainerService
            | Self::WrongRepovecdExecStart
            | Self::RepovecdWrongUser
            | Self::RepovecdMissingGroup => self.expected_repovecd_error(),
            Self::MissingMcpdRequiresQdrant
            | Self::MissingMcpdRequiresRepovecd
            | Self::MissingMcpdAfterQdrant
            | Self::MissingMcpdAfterRepovecd
            | Self::WrongMcpdExecStart
            | Self::McpdWrongWorkingDirectory
            | Self::McpdMissingEnvironment => self.expected_mcpd_error(),
        }
    }

    fn expected_target_error(self) -> SystemdUnitError {
        match self {
            Self::InvalidLine => SystemdUnitError::InvalidLine {
                unit: "repovec.target",
                line_number: 2,
                line: String::from("not valid"),
            },
            Self::PropertyBeforeSection => SystemdUnitError::PropertyBeforeSection {
                unit: "repovecd.service",
                line_number: 1,
                line: String::from("Requires=qdrant.service"),
            },
            Self::MissingTargetUnitSection => {
                SystemdUnitError::MissingSection { unit: "repovec.target", section: "Unit" }
            }
            Self::MissingTargetInstallSection => {
                SystemdUnitError::MissingSection { unit: "repovec.target", section: "Install" }
            }
            Self::MissingTargetWantedBy => missing_install("repovec.target", "multi-user.target"),
            Self::MissingTargetWantsQdrant => missing("repovec.target", "Wants", "qdrant.service"),
            Self::TargetUsesQdrantContainer => {
                quadlet_source("repovec.target", "Wants", "qdrant.container")
            }
            Self::MissingTargetWantsRepovecd => {
                missing("repovec.target", "Wants", "repovecd.service")
            }
            Self::MissingTargetWantsMcpd => {
                missing("repovec.target", "Wants", "repovec-mcpd.service")
            }
            Self::MissingTargetWantsCloudflared => {
                missing("repovec.target", "Wants", "cloudflared.service")
            }
            _ => panic!("target error called for non-target scenario"),
        }
    }

    fn expected_repovecd_error(self) -> SystemdUnitError {
        match self {
            Self::MissingRepovecdRequiresQdrant => {
                missing("repovecd.service", "Requires", "qdrant.service")
            }
            Self::MissingRepovecdAfterQdrant => {
                missing("repovecd.service", "After", "qdrant.service")
            }
            Self::RepovecdUsesQdrantContainerService => {
                quadlet_source("repovecd.service", "Requires", "qdrant.container.service")
            }
            Self::WrongRepovecdExecStart => {
                exec_start("repovecd.service", "/usr/bin/repovecd", "/usr/bin/otherd")
            }
            Self::RepovecdWrongUser => {
                service_directive("repovecd.service", "User", "repovec", "root")
            }
            Self::RepovecdMissingGroup => {
                service_directive("repovecd.service", "Group", "repovec", "")
            }
            _ => panic!("repovecd error called for non-repovecd scenario"),
        }
    }

    fn expected_mcpd_error(self) -> SystemdUnitError {
        match self {
            Self::MissingMcpdRequiresQdrant => {
                missing("repovec-mcpd.service", "Requires", "qdrant.service")
            }
            Self::MissingMcpdRequiresRepovecd => {
                missing("repovec-mcpd.service", "Requires", "repovecd.service")
            }
            Self::MissingMcpdAfterQdrant => {
                missing("repovec-mcpd.service", "After", "qdrant.service")
            }
            Self::MissingMcpdAfterRepovecd => {
                missing("repovec-mcpd.service", "After", "repovecd.service")
            }
            Self::WrongMcpdExecStart => {
                exec_start("repovec-mcpd.service", "/usr/bin/repovec-mcpd", "/usr/bin/other-mcpd")
            }
            Self::McpdWrongWorkingDirectory => service_directive(
                "repovec-mcpd.service",
                "WorkingDirectory",
                "/var/lib/repovec",
                "/tmp",
            ),
            Self::McpdMissingEnvironment => service_directive(
                "repovec-mcpd.service",
                "Environment",
                "HOME=/var/lib/repovec",
                "",
            ),
            _ => panic!("repovec-mcpd error called for non-mcpd scenario"),
        }
    }
}

#[fixture]
fn checked_in_unit_set() -> UnitSet {
    UnitSet {
        target: checked_in_repovec_target().to_owned(),
        repovecd: checked_in_repovecd_service().to_owned(),
        mcpd: checked_in_repovec_mcpd_service().to_owned(),
    }
}

#[test]
fn checked_in_systemd_units_remain_valid() {
    validate_checked_in_systemd_units()
        .expect("the checked-in repovec systemd unit set should remain valid");
}

#[test]
fn semicolon_comments_are_ignored() {
    let target = checked_in_repovec_target()
        .replace("[Unit]\n", "[Unit]\n; systemd accepts semicolon comments\n");

    validate_systemd_units(
        &target,
        checked_in_repovecd_service(),
        checked_in_repovec_mcpd_service(),
    )
    .expect("semicolon comments should be ignored");
}

#[rstest]
#[case::invalid_line(ValidationScenario::InvalidLine)]
#[case::property_before_section(ValidationScenario::PropertyBeforeSection)]
#[case::missing_target_unit_section(ValidationScenario::MissingTargetUnitSection)]
#[case::missing_target_install_section(ValidationScenario::MissingTargetInstallSection)]
#[case::missing_target_wanted_by(ValidationScenario::MissingTargetWantedBy)]
#[case::missing_target_wants_qdrant(ValidationScenario::MissingTargetWantsQdrant)]
#[case::target_uses_qdrant_container(ValidationScenario::TargetUsesQdrantContainer)]
#[case::missing_target_wants_repovecd(ValidationScenario::MissingTargetWantsRepovecd)]
#[case::missing_target_wants_mcpd(ValidationScenario::MissingTargetWantsMcpd)]
#[case::missing_target_wants_cloudflared(ValidationScenario::MissingTargetWantsCloudflared)]
#[case::missing_repovecd_requires_qdrant(ValidationScenario::MissingRepovecdRequiresQdrant)]
#[case::missing_repovecd_after_qdrant(ValidationScenario::MissingRepovecdAfterQdrant)]
#[case::repovecd_uses_qdrant_container_service(
    ValidationScenario::RepovecdUsesQdrantContainerService
)]
#[case::wrong_repovecd_exec_start(ValidationScenario::WrongRepovecdExecStart)]
#[case::repovecd_wrong_user(ValidationScenario::RepovecdWrongUser)]
#[case::repovecd_missing_group(ValidationScenario::RepovecdMissingGroup)]
#[case::missing_mcpd_requires_qdrant(ValidationScenario::MissingMcpdRequiresQdrant)]
#[case::missing_mcpd_requires_repovecd(ValidationScenario::MissingMcpdRequiresRepovecd)]
#[case::missing_mcpd_after_qdrant(ValidationScenario::MissingMcpdAfterQdrant)]
#[case::missing_mcpd_after_repovecd(ValidationScenario::MissingMcpdAfterRepovecd)]
#[case::wrong_mcpd_exec_start(ValidationScenario::WrongMcpdExecStart)]
#[case::mcpd_wrong_working_directory(ValidationScenario::McpdWrongWorkingDirectory)]
#[case::mcpd_missing_environment(ValidationScenario::McpdMissingEnvironment)]
fn validated_systemd_unit_violations_match_expected_variant_and_diagnostic_snapshots(
    checked_in_unit_set: UnitSet,
    #[case] scenario: ValidationScenario,
) {
    let units = scenario.mutate(checked_in_unit_set);
    let Err(err) = validate_systemd_units(&units.target, &units.repovecd, &units.mcpd) else {
        panic!("expected {scenario:?} validation to fail");
    };

    assert_eq!(err, scenario.expected_error());
    assert_eq!(err.to_string(), expected_diagnostic(scenario));
}

fn missing(unit: &'static str, key: &'static str, dependency: &'static str) -> SystemdUnitError {
    SystemdUnitError::MissingDependency { unit, section: "Unit", key, dependency }
}

fn missing_install(unit: &'static str, dependency: &'static str) -> SystemdUnitError {
    SystemdUnitError::MissingDependency { unit, section: "Install", key: "WantedBy", dependency }
}

fn quadlet_source(unit: &'static str, key: &'static str, dependency: &str) -> SystemdUnitError {
    SystemdUnitError::UsesQuadletSourceDependency {
        unit,
        section: "Unit",
        key,
        dependency: dependency.to_owned(),
    }
}

fn exec_start(unit: &'static str, expected: &'static str, actual: &str) -> SystemdUnitError {
    SystemdUnitError::IncorrectExecStart { unit, expected, actual: actual.to_owned() }
}

fn service_directive(
    unit: &'static str,
    key: &'static str,
    expected: &'static str,
    actual: &str,
) -> SystemdUnitError {
    SystemdUnitError::IncorrectServiceDirective { unit, key, expected, actual: actual.to_owned() }
}
