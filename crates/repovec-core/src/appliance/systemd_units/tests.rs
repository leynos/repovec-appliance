//! Contract tests for `validate_systemd_units`: deterministic mutations of the
//! shipped systemd units plus literal diagnostic assertions.

#[path = "tests/diagnostics.rs"]
mod diagnostics;
#[path = "tests/error_builders.rs"]
mod error_builders;
#[path = "tests/expected_errors.rs"]
mod expected_errors;
#[path = "tests/passing.rs"]
mod passing;
#[path = "tests/unit_set.rs"]
mod unit_set;

use diagnostics::expected_diagnostic;
use rstest::{fixture, rstest};
use unit_set::{UnitFile, UnitSet};

use super::{
    checked_in_repovec_mcpd_service, checked_in_repovec_target, checked_in_repovecd_service,
    validate_systemd_units,
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
    MissingRepovecdServiceSection,
    MissingRepovecdRequiresQdrant,
    MissingRepovecdAfterQdrant,
    RepovecdUsesQdrantContainerService,
    WrongRepovecdExecStart,
    RepovecdWrongUser,
    RepovecdMissingGroup,
    RepovecdWrongWorkingDirectory,
    RepovecdMissingEnvironment,
    MissingMcpdServiceSection,
    MissingMcpdRequiresQdrant,
    MissingMcpdRequiresRepovecd,
    MissingMcpdAfterQdrant,
    MissingMcpdAfterRepovecd,
    WrongMcpdExecStart,
    McpdWrongUser,
    McpdMissingGroup,
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
            | Self::MissingRepovecdServiceSection
            | Self::MissingRepovecdRequiresQdrant
            | Self::MissingRepovecdAfterQdrant
            | Self::RepovecdUsesQdrantContainerService
            | Self::WrongRepovecdExecStart
            | Self::RepovecdWrongUser
            | Self::RepovecdMissingGroup
            | Self::RepovecdWrongWorkingDirectory
            | Self::RepovecdMissingEnvironment => self.mutate_repovecd(&mut units),
            Self::MissingMcpdServiceSection
            | Self::MissingMcpdRequiresQdrant
            | Self::MissingMcpdRequiresRepovecd
            | Self::MissingMcpdAfterQdrant
            | Self::MissingMcpdAfterRepovecd
            | Self::WrongMcpdExecStart
            | Self::McpdWrongUser
            | Self::McpdMissingGroup
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
            Self::MissingRepovecdServiceSection => {
                units.remove_line(UnitFile::Repovecd, "[Service]\n");
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
            Self::RepovecdWrongWorkingDirectory => units.replace_token(
                UnitFile::Repovecd,
                "WorkingDirectory=/var/lib/repovec",
                "WorkingDirectory=/tmp",
            ),
            Self::RepovecdMissingEnvironment => {
                units.remove_line(UnitFile::Repovecd, "Environment=HOME=/var/lib/repovec\n");
            }
            _ => panic!("repovecd mutation called for non-repovecd scenario"),
        }
    }

    fn mutate_mcpd(self, units: &mut UnitSet) {
        match self {
            Self::MissingMcpdServiceSection => {
                units.remove_line(UnitFile::Mcpd, "[Service]\n");
            }
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
            Self::McpdWrongUser => {
                units.replace_token(UnitFile::Mcpd, "User=repovec", "User=root");
            }
            Self::McpdMissingGroup => {
                units.remove_line(UnitFile::Mcpd, "Group=repovec\n");
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
}

#[fixture]
fn checked_in_unit_set() -> UnitSet {
    UnitSet {
        target: checked_in_repovec_target().to_owned(),
        repovecd: checked_in_repovecd_service().to_owned(),
        mcpd: checked_in_repovec_mcpd_service().to_owned(),
    }
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
#[case::missing_repovecd_service_section(ValidationScenario::MissingRepovecdServiceSection)]
#[case::missing_repovecd_requires_qdrant(ValidationScenario::MissingRepovecdRequiresQdrant)]
#[case::missing_repovecd_after_qdrant(ValidationScenario::MissingRepovecdAfterQdrant)]
#[case::repovecd_uses_qdrant_container_service(
    ValidationScenario::RepovecdUsesQdrantContainerService
)]
#[case::wrong_repovecd_exec_start(ValidationScenario::WrongRepovecdExecStart)]
#[case::repovecd_wrong_user(ValidationScenario::RepovecdWrongUser)]
#[case::repovecd_missing_group(ValidationScenario::RepovecdMissingGroup)]
#[case::repovecd_wrong_working_directory(ValidationScenario::RepovecdWrongWorkingDirectory)]
#[case::repovecd_missing_environment(ValidationScenario::RepovecdMissingEnvironment)]
#[case::missing_mcpd_service_section(ValidationScenario::MissingMcpdServiceSection)]
#[case::missing_mcpd_requires_qdrant(ValidationScenario::MissingMcpdRequiresQdrant)]
#[case::missing_mcpd_requires_repovecd(ValidationScenario::MissingMcpdRequiresRepovecd)]
#[case::missing_mcpd_after_qdrant(ValidationScenario::MissingMcpdAfterQdrant)]
#[case::missing_mcpd_after_repovecd(ValidationScenario::MissingMcpdAfterRepovecd)]
#[case::wrong_mcpd_exec_start(ValidationScenario::WrongMcpdExecStart)]
#[case::mcpd_wrong_user(ValidationScenario::McpdWrongUser)]
#[case::mcpd_missing_group(ValidationScenario::McpdMissingGroup)]
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
