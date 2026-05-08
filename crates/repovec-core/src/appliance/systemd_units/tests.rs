//! Contract tests for `validate_systemd_units`: deterministic mutations of the
//! shipped systemd units plus committed `insta` diagnostics.

use rstest::{fixture, rstest};

use super::{
    SystemdUnitError, checked_in_repovec_mcpd_service, checked_in_repovec_target,
    checked_in_repovecd_service, validate_checked_in_systemd_units, validate_systemd_units,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum UnitFile {
    Target,
    Repovecd,
    Mcpd,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ValidationScenario {
    InvalidLine,
    PropertyBeforeSection,
    MissingTargetUnitSection,
    MissingTargetWantsQdrant,
    TargetUsesQdrantContainer,
    MissingTargetWantsRepovecd,
    MissingTargetWantsMcpd,
    MissingTargetWantsCloudflared,
    MissingRepovecdRequiresQdrant,
    MissingRepovecdAfterQdrant,
    RepovecdUsesQdrantContainerService,
    WrongRepovecdExecStart,
    MissingMcpdRequiresQdrant,
    MissingMcpdRequiresRepovecd,
    MissingMcpdAfterQdrant,
    MissingMcpdAfterRepovecd,
    WrongMcpdExecStart,
}

impl ValidationScenario {
    fn mutate(self, mut units: UnitSet) -> UnitSet {
        match self {
            Self::InvalidLine => units.replace_file(UnitFile::Target, "[Unit]\nnot valid\n"),
            Self::PropertyBeforeSection => {
                units.replace_file(UnitFile::Repovecd, "Requires=qdrant.service\n[Unit]\n");
            }
            Self::MissingTargetUnitSection => {
                units.replace_file(UnitFile::Target, "[Install]\nWantedBy=multi-user.target\n");
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
        }

        units
    }

    fn expected_error(self) -> SystemdUnitError {
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
        }
    }

    fn expected_diagnostic(self) -> &'static str {
        match self {
            Self::InvalidLine => "invalid systemd line in repovec.target at 2: not valid",
            Self::PropertyBeforeSection => concat!(
                "systemd property before section in repovecd.service on line 1: ",
                "Requires=qdrant.service",
            ),
            Self::MissingTargetUnitSection => "repovec.target is missing [Unit]",
            Self::MissingTargetWantsQdrant => {
                "repovec.target is missing Wants=qdrant.service in [Unit]"
            }
            Self::TargetUsesQdrantContainer => concat!(
                "repovec.target must depend on qdrant.service, not qdrant.container, ",
                "in [Unit] Wants",
            ),
            Self::MissingTargetWantsRepovecd => {
                "repovec.target is missing Wants=repovecd.service in [Unit]"
            }
            Self::MissingTargetWantsMcpd => {
                "repovec.target is missing Wants=repovec-mcpd.service in [Unit]"
            }
            Self::MissingTargetWantsCloudflared => {
                "repovec.target is missing Wants=cloudflared.service in [Unit]"
            }
            Self::MissingRepovecdRequiresQdrant => {
                "repovecd.service is missing Requires=qdrant.service in [Unit]"
            }
            Self::MissingRepovecdAfterQdrant => {
                "repovecd.service is missing After=qdrant.service in [Unit]"
            }
            Self::RepovecdUsesQdrantContainerService => concat!(
                "repovecd.service must depend on qdrant.service, not ",
                "qdrant.container.service, in [Unit] Requires",
            ),
            Self::WrongRepovecdExecStart => {
                "repovecd.service must use ExecStart=/usr/bin/repovecd: /usr/bin/otherd"
            }
            Self::MissingMcpdRequiresQdrant => {
                "repovec-mcpd.service is missing Requires=qdrant.service in [Unit]"
            }
            Self::MissingMcpdRequiresRepovecd => {
                "repovec-mcpd.service is missing Requires=repovecd.service in [Unit]"
            }
            Self::MissingMcpdAfterQdrant => {
                "repovec-mcpd.service is missing After=qdrant.service in [Unit]"
            }
            Self::MissingMcpdAfterRepovecd => {
                "repovec-mcpd.service is missing After=repovecd.service in [Unit]"
            }
            Self::WrongMcpdExecStart => concat!(
                "repovec-mcpd.service must use ExecStart=/usr/bin/repovec-mcpd: ",
                "/usr/bin/other-mcpd",
            ),
        }
    }
}

#[derive(Clone, Debug)]
struct UnitSet {
    target: String,
    repovecd: String,
    mcpd: String,
}

impl UnitSet {
    fn replace_file(&mut self, file: UnitFile, contents: &str) {
        *self.file_mut(file) = contents.to_owned();
    }

    fn remove_line(&mut self, file: UnitFile, line: &str) {
        let contents = self.file_mut(file);
        *contents = contents.replace(line, "");
    }

    fn replace_token(&mut self, file: UnitFile, from: &str, to: &str) {
        let contents = self.file_mut(file);
        *contents = contents.replace(from, to);
    }

    fn remove_token(&mut self, file: UnitFile, key: &str, token: &str) {
        let contents = self.file_mut(file);
        let mut lines = contents
            .lines()
            .map(|line| remove_token_from_line(line, key, token))
            .collect::<Vec<_>>()
            .join("\n");
        lines.push('\n');
        *contents = lines;
    }

    fn file_mut(&mut self, file: UnitFile) -> &mut String {
        match file {
            UnitFile::Target => &mut self.target,
            UnitFile::Repovecd => &mut self.repovecd,
            UnitFile::Mcpd => &mut self.mcpd,
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

#[rstest]
#[case::invalid_line(ValidationScenario::InvalidLine)]
#[case::property_before_section(ValidationScenario::PropertyBeforeSection)]
#[case::missing_target_unit_section(ValidationScenario::MissingTargetUnitSection)]
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
#[case::missing_mcpd_requires_qdrant(ValidationScenario::MissingMcpdRequiresQdrant)]
#[case::missing_mcpd_requires_repovecd(ValidationScenario::MissingMcpdRequiresRepovecd)]
#[case::missing_mcpd_after_qdrant(ValidationScenario::MissingMcpdAfterQdrant)]
#[case::missing_mcpd_after_repovecd(ValidationScenario::MissingMcpdAfterRepovecd)]
#[case::wrong_mcpd_exec_start(ValidationScenario::WrongMcpdExecStart)]
fn validated_systemd_unit_violations_match_expected_variant_and_diagnostic_snapshots(
    checked_in_unit_set: UnitSet,
    #[case] scenario: ValidationScenario,
) {
    let units = scenario.mutate(checked_in_unit_set);
    let Err(err) = validate_systemd_units(&units.target, &units.repovecd, &units.mcpd) else {
        panic!("expected {scenario:?} validation to fail");
    };

    assert_eq!(err, scenario.expected_error());
    assert_eq!(err.to_string(), scenario.expected_diagnostic());
}

fn remove_token_from_line(line: &str, key: &str, token: &str) -> String {
    let Some(value) = line.strip_prefix(key) else {
        return line.to_owned();
    };
    let retained = value.split_whitespace().filter(|candidate| *candidate != token);

    format!("{key}{}", retained.collect::<Vec<_>>().join(" "))
}

fn missing(unit: &'static str, key: &'static str, dependency: &'static str) -> SystemdUnitError {
    SystemdUnitError::MissingDependency { unit, section: "Unit", key, dependency }
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
