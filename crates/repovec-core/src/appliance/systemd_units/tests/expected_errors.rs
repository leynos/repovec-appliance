//! Typed expected-error mapping for systemd unit validator tests.

use super::{
    ValidationScenario,
    error_builders::{exec_start, missing, missing_install, quadlet_source, service_directive},
};
use crate::appliance::systemd_units::SystemdUnitError;

impl ValidationScenario {
    pub(super) fn expected_error(self) -> SystemdUnitError {
        match self {
            Self::InvalidLine
            | Self::MissingTargetUnitSection
            | Self::MissingTargetInstallSection
            | Self::MissingTargetWantedBy
            | Self::MissingTargetWantsQdrant
            | Self::TargetUsesQdrantContainer
            | Self::MissingTargetWantsRepovecd
            | Self::MissingTargetWantsMcpd
            | Self::MissingTargetWantsCloudflared => self.expected_target_error(),
            Self::PropertyBeforeSection
            | Self::MissingRepovecdServiceSection
            | Self::MissingRepovecdRequiresQdrant
            | Self::MissingRepovecdAfterQdrant
            | Self::RepovecdUsesQdrantContainerService
            | Self::WrongRepovecdExecStart
            | Self::RepovecdWrongUser
            | Self::RepovecdMissingGroup
            | Self::RepovecdWrongWorkingDirectory
            | Self::RepovecdMissingEnvironment => self.expected_repovecd_error(),
            Self::MissingMcpdServiceSection
            | Self::MissingMcpdRequiresQdrant
            | Self::MissingMcpdRequiresRepovecd
            | Self::MissingMcpdAfterQdrant
            | Self::MissingMcpdAfterRepovecd
            | Self::WrongMcpdExecStart
            | Self::McpdWrongUser
            | Self::McpdMissingGroup
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
            Self::PropertyBeforeSection => SystemdUnitError::PropertyBeforeSection {
                unit: "repovecd.service",
                line_number: 1,
                line: String::from("Requires=qdrant.service"),
            },
            Self::MissingRepovecdServiceSection => {
                SystemdUnitError::MissingSection { unit: "repovecd.service", section: "Service" }
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
            Self::RepovecdWrongUser => {
                service_directive("repovecd.service", "User", "repovec", "root")
            }
            Self::RepovecdMissingGroup => {
                service_directive("repovecd.service", "Group", "repovec", "")
            }
            Self::RepovecdWrongWorkingDirectory => service_directive(
                "repovecd.service",
                "WorkingDirectory",
                "/var/lib/repovec",
                "/tmp",
            ),
            Self::RepovecdMissingEnvironment => {
                service_directive("repovecd.service", "Environment", "HOME=/var/lib/repovec", "")
            }
            _ => panic!("repovecd error called for non-repovecd scenario"),
        }
    }

    fn expected_mcpd_error(self) -> SystemdUnitError {
        match self {
            Self::MissingMcpdServiceSection => SystemdUnitError::MissingSection {
                unit: "repovec-mcpd.service",
                section: "Service",
            },
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
            Self::McpdWrongUser => {
                service_directive("repovec-mcpd.service", "User", "repovec", "root")
            }
            Self::McpdMissingGroup => {
                service_directive("repovec-mcpd.service", "Group", "repovec", "")
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
