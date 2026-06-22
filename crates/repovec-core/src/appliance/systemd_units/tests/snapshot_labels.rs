//! Snapshot labels for systemd unit validator display diagnostics.

use super::ValidationScenario;

impl ValidationScenario {
    pub(super) fn snapshot_label(self) -> &'static str {
        match self {
            Self::InvalidLine => "invalid_line_display",
            Self::MissingTargetUnitSection
            | Self::MissingTargetInstallSection
            | Self::MissingTargetWantedBy
            | Self::MissingTargetWantsQdrant
            | Self::TargetUsesQdrantContainer
            | Self::MissingTargetWantsRepovecd
            | Self::MissingTargetWantsMcpd
            | Self::MissingTargetWantsCloudflared => self.target_snapshot_label(),
            Self::PropertyBeforeSection
            | Self::MissingRepovecdServiceSection
            | Self::MissingRepovecdRequiresQdrant
            | Self::MissingRepovecdAfterQdrant
            | Self::RepovecdUsesQdrantContainerService
            | Self::WrongRepovecdExecStart
            | Self::RepovecdWrongUser
            | Self::RepovecdMissingGroup
            | Self::RepovecdWrongWorkingDirectory
            | Self::RepovecdMissingEnvironment => self.repovecd_snapshot_label(),
            Self::MissingMcpdServiceSection
            | Self::MissingMcpdRequiresQdrant
            | Self::MissingMcpdRequiresRepovecd
            | Self::MissingMcpdAfterQdrant
            | Self::MissingMcpdAfterRepovecd
            | Self::WrongMcpdExecStart
            | Self::McpdWrongUser
            | Self::McpdMissingGroup
            | Self::McpdWrongWorkingDirectory
            | Self::McpdMissingEnvironment => self.mcpd_snapshot_label(),
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
            | Self::GrepaiTemplateLogsStderrToFile => self.grepai_template_snapshot_label(),
        }
    }

    fn target_snapshot_label(self) -> &'static str {
        match self {
            Self::MissingTargetUnitSection => "missing_target_unit_section_display",
            Self::MissingTargetInstallSection => "missing_target_install_section_display",
            Self::MissingTargetWantedBy => "missing_target_wanted_by_display",
            Self::MissingTargetWantsQdrant => "missing_target_wants_qdrant_display",
            Self::TargetUsesQdrantContainer => "target_uses_qdrant_container_display",
            Self::MissingTargetWantsRepovecd => "missing_target_wants_repovecd_display",
            Self::MissingTargetWantsMcpd => "missing_target_wants_mcpd_display",
            Self::MissingTargetWantsCloudflared => "missing_target_wants_cloudflared_display",
            _ => panic!("target snapshot label called for non-target scenario"),
        }
    }

    fn repovecd_snapshot_label(self) -> &'static str {
        match self {
            Self::PropertyBeforeSection => "property_before_section_display",
            Self::MissingRepovecdServiceSection => "missing_repovecd_service_section_display",
            Self::MissingRepovecdRequiresQdrant => "missing_repovecd_requires_qdrant_display",
            Self::MissingRepovecdAfterQdrant => "missing_repovecd_after_qdrant_display",
            Self::RepovecdUsesQdrantContainerService => {
                "repovecd_uses_qdrant_container_service_display"
            }
            Self::WrongRepovecdExecStart => "wrong_repovecd_exec_start_display",
            Self::RepovecdWrongUser => "repovecd_wrong_user_display",
            Self::RepovecdMissingGroup => "repovecd_missing_group_display",
            Self::RepovecdWrongWorkingDirectory => "repovecd_wrong_working_directory_display",
            Self::RepovecdMissingEnvironment => "repovecd_missing_environment_display",
            _ => panic!("repovecd snapshot label called for non-repovecd scenario"),
        }
    }

    fn mcpd_snapshot_label(self) -> &'static str {
        match self {
            Self::MissingMcpdServiceSection => "missing_mcpd_service_section_display",
            Self::MissingMcpdRequiresQdrant => "missing_mcpd_requires_qdrant_display",
            Self::MissingMcpdRequiresRepovecd => "missing_mcpd_requires_repovecd_display",
            Self::MissingMcpdAfterQdrant => "missing_mcpd_after_qdrant_display",
            Self::MissingMcpdAfterRepovecd => "missing_mcpd_after_repovecd_display",
            Self::WrongMcpdExecStart => "wrong_mcpd_exec_start_display",
            Self::McpdWrongUser => "mcpd_wrong_user_display",
            Self::McpdMissingGroup => "mcpd_missing_group_display",
            Self::McpdWrongWorkingDirectory => "mcpd_wrong_working_directory_display",
            Self::McpdMissingEnvironment => "mcpd_missing_environment_display",
            _ => panic!("repovec-mcpd snapshot label called for non-mcpd scenario"),
        }
    }

    fn grepai_template_snapshot_label(self) -> &'static str {
        match self {
            Self::MissingGrepaiTemplateUnitSection => {
                "missing_grepai_template_unit_section_display"
            }
            Self::MissingGrepaiTemplateServiceSection => {
                "missing_grepai_template_service_section_display"
            }
            Self::MissingGrepaiTemplateInstallSection => {
                "missing_grepai_template_install_section_display"
            }
            Self::MissingGrepaiTemplateRequiresQdrant => {
                "missing_grepai_template_requires_qdrant_display"
            }
            Self::MissingGrepaiTemplateRequiresRepovecd => {
                "missing_grepai_template_requires_repovecd_display"
            }
            Self::MissingGrepaiTemplateAfterQdrant => {
                "missing_grepai_template_after_qdrant_display"
            }
            Self::MissingGrepaiTemplateAfterRepovecd => {
                "missing_grepai_template_after_repovecd_display"
            }
            Self::GrepaiTemplateUsesQdrantContainer => {
                "grepai_template_uses_qdrant_container_display"
            }
            Self::MissingGrepaiTemplatePartOfTarget => {
                "missing_grepai_template_part_of_target_display"
            }
            Self::MissingGrepaiTemplateWantedByTarget => {
                "missing_grepai_template_wanted_by_target_display"
            }
            Self::WrongGrepaiTemplateType => "wrong_grepai_template_type_display",
            Self::WrongGrepaiTemplateExecStart => "wrong_grepai_template_exec_start_display",
            Self::GrepaiTemplateWrongUser => "grepai_template_wrong_user_display",
            Self::GrepaiTemplateMissingGroup => "grepai_template_missing_group_display",
            Self::GrepaiTemplateWrongWorkingDirectory => {
                "grepai_template_wrong_working_directory_display"
            }
            Self::GrepaiTemplateMissingEnvironment => "grepai_template_missing_environment_display",
            Self::GrepaiTemplateWrongRestartPolicy => {
                "grepai_template_wrong_restart_policy_display"
            }
            Self::GrepaiTemplateWrongRestartDelay => "grepai_template_wrong_restart_delay_display",
            Self::GrepaiTemplateLogsStdoutToFile => "grepai_template_logs_stdout_to_file_display",
            Self::GrepaiTemplateLogsStderrToFile => "grepai_template_logs_stderr_to_file_display",
            _ => panic!("grepai template snapshot label called for non-template scenario"),
        }
    }
}
