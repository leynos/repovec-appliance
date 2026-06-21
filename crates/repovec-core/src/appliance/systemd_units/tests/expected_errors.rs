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
            Self::MissingGrepaiTemplateUnitSection
            | Self::MissingGrepaiTemplateServiceSection
            | Self::MissingGrepaiTemplateInstallSection
            | Self::MissingGrepaiTemplateRequiresQdrant
            | Self::MissingGrepaiTemplateRequiresRepovecd
            | Self::MissingGrepaiTemplateAfterQdrant
            | Self::MissingGrepaiTemplateAfterRepovecd
            | Self::GrepaiTemplateUsesQdrantContainer
            | Self::MissingGrepaiTemplatePartOfTarget
            | Self::MissingGrepaiTemplateWantedByTarget
            | Self::WrongGrepaiTemplateType
            | Self::WrongGrepaiTemplateExecStart
            | Self::GrepaiTemplateWrongUser
            | Self::GrepaiTemplateMissingGroup
            | Self::GrepaiTemplateWrongWorkingDirectory
            | Self::GrepaiTemplateMissingEnvironment
            | Self::GrepaiTemplateWrongRestartPolicy
            | Self::GrepaiTemplateWrongRestartDelay
            | Self::GrepaiTemplateLogsStdoutToFile
            | Self::GrepaiTemplateLogsStderrToFile => self.expected_grepai_template_error(),
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

    fn expected_grepai_template_error(self) -> SystemdUnitError {
        match self {
            Self::MissingGrepaiTemplateUnitSection => SystemdUnitError::MissingSection {
                unit: "repovec-grepai@.service",
                section: "Unit",
            },
            Self::MissingGrepaiTemplateServiceSection => SystemdUnitError::MissingSection {
                unit: "repovec-grepai@.service",
                section: "Service",
            },
            Self::MissingGrepaiTemplateInstallSection => SystemdUnitError::MissingSection {
                unit: "repovec-grepai@.service",
                section: "Install",
            },
            Self::MissingGrepaiTemplateRequiresQdrant
            | Self::MissingGrepaiTemplateRequiresRepovecd
            | Self::MissingGrepaiTemplateAfterQdrant
            | Self::MissingGrepaiTemplateAfterRepovecd
            | Self::GrepaiTemplateUsesQdrantContainer
            | Self::MissingGrepaiTemplatePartOfTarget
            | Self::MissingGrepaiTemplateWantedByTarget => {
                self.expected_grepai_template_dependency_error()
            }
            Self::WrongGrepaiTemplateType
            | Self::WrongGrepaiTemplateExecStart
            | Self::GrepaiTemplateWrongUser
            | Self::GrepaiTemplateMissingGroup
            | Self::GrepaiTemplateWrongWorkingDirectory
            | Self::GrepaiTemplateMissingEnvironment
            | Self::GrepaiTemplateWrongRestartPolicy
            | Self::GrepaiTemplateWrongRestartDelay
            | Self::GrepaiTemplateLogsStdoutToFile
            | Self::GrepaiTemplateLogsStderrToFile => self.expected_grepai_template_service_error(),
            _ => panic!("grepai template error called for non-template scenario"),
        }
    }

    fn expected_grepai_template_dependency_error(self) -> SystemdUnitError {
        match self {
            Self::MissingGrepaiTemplateRequiresQdrant => {
                missing("repovec-grepai@.service", "Requires", "qdrant.service")
            }
            Self::MissingGrepaiTemplateRequiresRepovecd => {
                missing("repovec-grepai@.service", "Requires", "repovecd.service")
            }
            Self::MissingGrepaiTemplateAfterQdrant => {
                missing("repovec-grepai@.service", "After", "qdrant.service")
            }
            Self::MissingGrepaiTemplateAfterRepovecd => {
                missing("repovec-grepai@.service", "After", "repovecd.service")
            }
            Self::GrepaiTemplateUsesQdrantContainer => {
                quadlet_source("repovec-grepai@.service", "Requires", "qdrant.container")
            }
            Self::MissingGrepaiTemplatePartOfTarget => {
                missing("repovec-grepai@.service", "PartOf", "repovec.target")
            }
            Self::MissingGrepaiTemplateWantedByTarget => SystemdUnitError::MissingDependency {
                unit: "repovec-grepai@.service",
                section: "Install",
                key: "WantedBy",
                dependency: "repovec.target",
            },
            _ => panic!("grepai template dependency error called for non-dependency scenario"),
        }
    }

    fn expected_grepai_template_service_error(self) -> SystemdUnitError {
        match self {
            Self::WrongGrepaiTemplateType => {
                service_directive("repovec-grepai@.service", "Type", "exec", "simple")
            }
            Self::WrongGrepaiTemplateExecStart => {
                exec_start("repovec-grepai@.service", "/usr/bin/grepai watch", "/usr/bin/grepai")
            }
            Self::GrepaiTemplateWrongUser => {
                service_directive("repovec-grepai@.service", "User", "repovec", "root")
            }
            Self::GrepaiTemplateMissingGroup => {
                service_directive("repovec-grepai@.service", "Group", "repovec", "")
            }
            Self::GrepaiTemplateWrongWorkingDirectory => service_directive(
                "repovec-grepai@.service",
                "WorkingDirectory",
                "/var/lib/repovec/worktrees/%I",
                "/var/lib/repovec",
            ),
            Self::GrepaiTemplateMissingEnvironment => service_directive(
                "repovec-grepai@.service",
                "Environment",
                "HOME=/var/lib/repovec",
                "",
            ),
            Self::GrepaiTemplateWrongRestartPolicy => {
                service_directive("repovec-grepai@.service", "Restart", "on-failure", "always")
            }
            Self::GrepaiTemplateWrongRestartDelay => {
                service_directive("repovec-grepai@.service", "RestartSec", "5s", "0")
            }
            Self::GrepaiTemplateLogsStdoutToFile => service_directive(
                "repovec-grepai@.service",
                "StandardOutput",
                "journal",
                "file:/var/log/repovec/grepai.log",
            ),
            Self::GrepaiTemplateLogsStderrToFile => service_directive(
                "repovec-grepai@.service",
                "StandardError",
                "journal",
                "file:/var/log/repovec/grepai.err",
            ),
            _ => panic!("grepai template service error called for non-service scenario"),
        }
    }
}
