//! Mutation scenarios for the systemd unit validator tests.
//!
//! [`super`] drives each scenario through a deterministic mutation of the
//! checked-in unit set and asserts the typed error and committed snapshot.

use super::unit_set::{UnitFile, UnitSet};

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
    MissingTargetWantsProvision,
    MissingProvisionUnitSection,
    MissingProvisionServiceSection,
    MissingProvisionInstallSection,
    MissingProvisionWantedByTarget,
    MissingProvisionAfterSysusers,
    MissingProvisionBeforeRepovecd,
    WrongProvisionSysusersExecStart,
    MissingProvisionType,
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
    MissingGrepaiTemplateUnitSection,
    MissingGrepaiTemplateServiceSection,
    MissingGrepaiTemplateInstallSection,
    MissingGrepaiTemplateRequiresQdrant,
    MissingGrepaiTemplateRequiresRepovecd,
    MissingGrepaiTemplateAfterQdrant,
    MissingGrepaiTemplateAfterRepovecd,
    GrepaiTemplateUsesQdrantContainer,
    MissingGrepaiTemplatePartOfTarget,
    MissingGrepaiTemplateWantedByTarget,
    WrongGrepaiTemplateType,
    WrongGrepaiTemplateExecStart,
    GrepaiTemplateWrongUser,
    GrepaiTemplateMissingGroup,
    GrepaiTemplateWrongWorkingDirectory,
    GrepaiTemplateMissingEnvironment,
    GrepaiTemplateWrongRestartPolicy,
    GrepaiTemplateWrongRestartDelay,
    GrepaiTemplateLogsStdoutToFile,
    GrepaiTemplateLogsStderrToFile,
}

impl ValidationScenario {
    pub(super) fn mutate(self, mut units: UnitSet) -> UnitSet {
        match self {
            Self::InvalidLine
            | Self::MissingTargetUnitSection
            | Self::MissingTargetInstallSection
            | Self::MissingTargetWantedBy
            | Self::MissingTargetWantsQdrant
            | Self::TargetUsesQdrantContainer
            | Self::MissingTargetWantsRepovecd
            | Self::MissingTargetWantsMcpd
            | Self::MissingTargetWantsCloudflared
            | Self::MissingTargetWantsProvision => self.mutate_target(&mut units),
            Self::MissingProvisionUnitSection
            | Self::MissingProvisionServiceSection
            | Self::MissingProvisionInstallSection
            | Self::MissingProvisionWantedByTarget
            | Self::MissingProvisionAfterSysusers
            | Self::MissingProvisionBeforeRepovecd
            | Self::WrongProvisionSysusersExecStart
            | Self::MissingProvisionType => self.mutate_provision(&mut units),
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
            | Self::GrepaiTemplateLogsStderrToFile => self.mutate_grepai_template(&mut units),
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
            Self::MissingTargetWantsProvision => {
                units.remove_token(UnitFile::Target, "Wants=", "repovec-provision.service");
            }
            _ => panic!("target mutation called for non-target scenario"),
        }
    }

    fn mutate_provision(self, units: &mut UnitSet) {
        match self {
            Self::MissingProvisionUnitSection => {
                units.replace_file(
                    UnitFile::Provision,
                    "[Service]\nType=oneshot\nRemainAfterExit=yes\n\n[Install]\nWantedBy=repovec.target\n",
                );
            }
            Self::MissingProvisionServiceSection => {
                units.replace_file(
                    UnitFile::Provision,
                    "[Unit]\nWants=systemd-sysusers.service\nAfter=systemd-sysusers.service\nBefore=repovec-qdrant-api-key.service qdrant.service repovecd.service repovec-mcpd.service\n\n[Install]\nWantedBy=repovec.target\n",
                );
            }
            Self::MissingProvisionInstallSection => {
                units.remove_line(UnitFile::Provision, "[Install]\n");
                units.remove_line(UnitFile::Provision, "WantedBy=repovec.target\n");
            }
            Self::MissingProvisionWantedByTarget => {
                units.remove_line(UnitFile::Provision, "WantedBy=repovec.target\n");
            }
            Self::MissingProvisionAfterSysusers => {
                units.remove_line(UnitFile::Provision, "After=systemd-sysusers.service\n");
            }
            Self::MissingProvisionBeforeRepovecd => {
                units.remove_token(UnitFile::Provision, "Before=", "repovecd.service");
            }
            Self::WrongProvisionSysusersExecStart => units.replace_token(
                UnitFile::Provision,
                "/usr/bin/systemd-sysusers /usr/lib/sysusers.d/repovec.conf",
                "/usr/bin/systemd-sysusers /usr/lib/sysusers.d/wrong.conf",
            ),
            Self::MissingProvisionType => {
                units.remove_line(UnitFile::Provision, "Type=oneshot\n");
            }
            _ => panic!("provision mutation called for non-provision scenario"),
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

    fn mutate_grepai_template(self, units: &mut UnitSet) {
        match self {
            Self::MissingGrepaiTemplateUnitSection => {
                self.mutate_grepai_template_unit_section(units);
            }
            Self::MissingGrepaiTemplateServiceSection => {
                units.remove_line(UnitFile::GrepaiTemplate, "[Service]\n");
            }
            Self::MissingGrepaiTemplateInstallSection => {
                units.remove_line(UnitFile::GrepaiTemplate, "[Install]\n");
                units.remove_line(UnitFile::GrepaiTemplate, "WantedBy=repovec.target\n");
            }
            Self::MissingGrepaiTemplateRequiresQdrant
            | Self::MissingGrepaiTemplateRequiresRepovecd
            | Self::MissingGrepaiTemplateAfterQdrant
            | Self::MissingGrepaiTemplateAfterRepovecd
            | Self::GrepaiTemplateUsesQdrantContainer
            | Self::MissingGrepaiTemplatePartOfTarget
            | Self::MissingGrepaiTemplateWantedByTarget => {
                self.mutate_grepai_template_dependencies(units);
            }
            Self::WrongGrepaiTemplateType => {
                units.replace_token(UnitFile::GrepaiTemplate, "Type=exec", "Type=simple");
            }
            Self::WrongGrepaiTemplateExecStart => units.replace_token(
                UnitFile::GrepaiTemplate,
                "/usr/bin/grepai watch",
                "/usr/bin/grepai",
            ),
            Self::GrepaiTemplateWrongUser => {
                units.replace_token(UnitFile::GrepaiTemplate, "User=repovec", "User=root");
            }
            Self::GrepaiTemplateMissingGroup => {
                units.remove_line(UnitFile::GrepaiTemplate, "Group=repovec\n");
            }
            Self::GrepaiTemplateWrongWorkingDirectory => units.replace_token(
                UnitFile::GrepaiTemplate,
                "WorkingDirectory=/var/lib/repovec/worktrees/%I",
                "WorkingDirectory=/var/lib/repovec",
            ),
            Self::GrepaiTemplateMissingEnvironment => {
                units.remove_line(UnitFile::GrepaiTemplate, "Environment=HOME=/var/lib/repovec\n");
            }
            Self::GrepaiTemplateWrongRestartPolicy => units.replace_token(
                UnitFile::GrepaiTemplate,
                "Restart=on-failure",
                "Restart=always",
            ),
            Self::GrepaiTemplateWrongRestartDelay => {
                units.replace_token(UnitFile::GrepaiTemplate, "RestartSec=5s", "RestartSec=0");
            }
            Self::GrepaiTemplateLogsStdoutToFile => units.replace_token(
                UnitFile::GrepaiTemplate,
                "StandardOutput=journal",
                "StandardOutput=file:/var/log/repovec/grepai.log",
            ),
            Self::GrepaiTemplateLogsStderrToFile => units.replace_token(
                UnitFile::GrepaiTemplate,
                "StandardError=journal",
                "StandardError=file:/var/log/repovec/grepai.err",
            ),
            _ => panic!("grepai template mutation called for non-template scenario"),
        }
    }
}
