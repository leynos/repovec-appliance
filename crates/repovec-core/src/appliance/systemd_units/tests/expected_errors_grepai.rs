//! Expected-error mapping for the grepai template unit scenarios.

use super::{
    error_builders::{exec_start, missing, quadlet_source, service_directive},
    scenarios::ValidationScenario,
};
use crate::appliance::systemd_units::SystemdUnitError;

impl ValidationScenario {
    pub(super) fn expected_grepai_template_error(self) -> SystemdUnitError {
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
            Self::WrongGrepaiTemplateExecStart => exec_start(
                "repovec-grepai@.service",
                "/usr/bin/grepai watch",
                "/usr/bin/grepai",
            ),
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
