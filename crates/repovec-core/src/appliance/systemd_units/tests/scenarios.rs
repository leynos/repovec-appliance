//! Mutation scenarios for the systemd unit validator tests.
//!
//! [`super`] drives each scenario through a deterministic mutation of the
//! checked-in unit set and asserts the typed error and committed snapshot.

use super::unit_set::{UnitFile, UnitSet};

mod scenario_catalog;

pub(super) use scenario_catalog::ValidationScenario;

#[derive(Clone, Copy)]
enum ServiceUnit {
    Repovecd,
    Mcpd,
}

impl ServiceUnit {
    fn file(self) -> UnitFile {
        match self {
            Self::Repovecd => UnitFile::Repovecd,
            Self::Mcpd => UnitFile::Mcpd,
        }
    }

    fn executable_pair(self) -> (&'static str, &'static str) {
        match self {
            Self::Repovecd => ("/usr/bin/repovecd", "/usr/bin/otherd"),
            Self::Mcpd => ("/usr/bin/repovec-mcpd", "/usr/bin/other-mcpd"),
        }
    }
}

impl ValidationScenario {
    pub(super) fn mutate(self, mut units: UnitSet) -> UnitSet {
        if self.mutate_service_scenario(&mut units) {
            return units;
        }

        if self.is_target_scenario() {
            self.mutate_target(&mut units);
        } else if self.is_provision_scenario() {
            self.mutate_provision(&mut units);
        } else if self.is_grepai_template_scenario() {
            self.mutate_grepai_template(&mut units);
        } else {
            panic!("validation scenario is missing a mutation dispatcher");
        }

        units
    }

    fn is_target_scenario(self) -> bool {
        matches!(
            self,
            Self::InvalidLine
                | Self::MissingTargetUnitSection
                | Self::MissingTargetInstallSection
                | Self::MissingTargetWantedBy
                | Self::MissingTargetWantsQdrant
                | Self::TargetUsesQdrantContainer
                | Self::MissingTargetWantsRepovecd
                | Self::MissingTargetWantsMcpd
                | Self::MissingTargetWantsCloudflared
                | Self::MissingTargetWantsProvision
        )
    }

    fn is_provision_scenario(self) -> bool {
        matches!(
            self,
            Self::MissingProvisionUnitSection
                | Self::MissingProvisionServiceSection
                | Self::MissingProvisionInstallSection
                | Self::MissingProvisionWantedByTarget
                | Self::MissingProvisionAfterSysusers
                | Self::MissingProvisionBeforeRepovecd
                | Self::WrongProvisionSysusersExecStart
                | Self::SwappedProvisionExecStartOrder
                | Self::ExtraProvisionExecStart
                | Self::MissingProvisionType
                | Self::ProvisionRemainsActive
                | Self::ProvisionRefusesManualStop
                | Self::ProvisionRefusesManualStopDisabled
        )
    }

    fn mutate_service_scenario(self, units: &mut UnitSet) -> bool {
        let Some(service) = self.service_unit() else {
            return false;
        };

        if !self.mutate_shared_service_settings(units, service) {
            self.mutate_service_dependencies(units, service);
        }

        true
    }

    fn service_unit(self) -> Option<ServiceUnit> {
        match self {
            Self::PropertyBeforeSection
            | Self::MissingRepovecdServiceSection
            | Self::MissingRepovecdRequiresQdrant
            | Self::MissingRepovecdRequiresProvision
            | Self::MissingRepovecdAfterQdrant
            | Self::RepovecdUsesQdrantContainerService
            | Self::WrongRepovecdExecStart
            | Self::RepovecdWrongUser
            | Self::RepovecdMissingGroup
            | Self::RepovecdWrongWorkingDirectory
            | Self::RepovecdMissingEnvironment => Some(ServiceUnit::Repovecd),
            Self::MissingMcpdServiceSection
            | Self::MissingMcpdRequiresQdrant
            | Self::MissingMcpdRequiresRepovecd
            | Self::MissingMcpdRequiresProvision
            | Self::MissingMcpdAfterQdrant
            | Self::MissingMcpdAfterRepovecd
            | Self::WrongMcpdExecStart
            | Self::McpdWrongUser
            | Self::McpdMissingGroup
            | Self::McpdWrongWorkingDirectory
            | Self::McpdMissingEnvironment => Some(ServiceUnit::Mcpd),
            _ => None,
        }
    }

    fn is_grepai_template_scenario(self) -> bool {
        matches!(
            self,
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
                | Self::GrepaiTemplateLogsStderrToFile
        )
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
                    "[Service]\nType=oneshot\nRemainAfterExit=no\n\n[Install]\nWantedBy=repovec.target\n",
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
            Self::SwappedProvisionExecStartOrder => units.replace_token(
                UnitFile::Provision,
                concat!(
                    "ExecStart=/usr/bin/systemd-sysusers /usr/lib/sysusers.d/repovec.conf\n",
                    "ExecStart=/usr/bin/systemd-tmpfiles --create /usr/lib/tmpfiles.d/repovec.conf\n",
                ),
                concat!(
                    "ExecStart=/usr/bin/systemd-tmpfiles --create /usr/lib/tmpfiles.d/repovec.conf\n",
                    "ExecStart=/usr/bin/systemd-sysusers /usr/lib/sysusers.d/repovec.conf\n",
                ),
            ),
            Self::ExtraProvisionExecStart => units.replace_token(
                UnitFile::Provision,
                "ExecStart=/usr/bin/systemd-tmpfiles --create /usr/lib/tmpfiles.d/repovec.conf\n",
                concat!(
                    "ExecStart=/usr/bin/systemd-tmpfiles --create /usr/lib/tmpfiles.d/repovec.conf\n",
                    "ExecStart=/usr/bin/systemd-tmpfiles --create /usr/lib/tmpfiles.d/extra.conf\n",
                ),
            ),
            Self::MissingProvisionType => {
                units.remove_line(UnitFile::Provision, "Type=oneshot\n");
            }
            Self::ProvisionRemainsActive
            | Self::ProvisionRefusesManualStop
            | Self::ProvisionRefusesManualStopDisabled => {
                self.mutate_provision_lifecycle_directives(units);
            }
            _ => panic!("provision mutation called for non-provision scenario"),
        }
    }

    fn mutate_shared_service_settings(self, units: &mut UnitSet, service: ServiceUnit) -> bool {
        let file = service.file();
        let (executable, replacement) = service.executable_pair();
        match self {
            Self::MissingRepovecdServiceSection | Self::MissingMcpdServiceSection => {
                units.remove_line(file, "[Service]\n");
            }
            Self::WrongRepovecdExecStart | Self::WrongMcpdExecStart => {
                units.replace_token(file, executable, replacement);
            }
            Self::RepovecdWrongUser | Self::McpdWrongUser => {
                units.replace_token(file, "User=repovec", "User=root");
            }
            Self::RepovecdMissingGroup | Self::McpdMissingGroup => {
                units.remove_line(file, "Group=repovec\n");
            }
            Self::RepovecdWrongWorkingDirectory | Self::McpdWrongWorkingDirectory => {
                units.replace_token(
                    file,
                    "WorkingDirectory=/var/lib/repovec",
                    "WorkingDirectory=/tmp",
                );
            }
            Self::RepovecdMissingEnvironment | Self::McpdMissingEnvironment => {
                units.remove_line(file, "Environment=HOME=/var/lib/repovec\n");
            }
            _ => return false,
        }

        true
    }

    fn mutate_service_dependencies(self, units: &mut UnitSet, service: ServiceUnit) {
        let file = service.file();
        match self {
            Self::PropertyBeforeSection => {
                units.replace_file(file, "Requires=qdrant.service\n[Unit]\n");
            }
            Self::MissingRepovecdRequiresQdrant => {
                units.remove_line(file, "Requires=qdrant.service\n");
            }
            Self::MissingRepovecdRequiresProvision | Self::MissingMcpdRequiresProvision => {
                units.remove_line(file, "Requires=repovec-provision.service\n");
            }
            Self::MissingRepovecdAfterQdrant => {
                units.remove_line(file, "After=qdrant.service\n");
            }
            Self::RepovecdUsesQdrantContainerService => {
                units.replace_token(file, "qdrant.service", "qdrant.container.service");
            }
            Self::MissingMcpdRequiresQdrant => {
                units.remove_token(file, "Requires=", "qdrant.service");
            }
            Self::MissingMcpdRequiresRepovecd => {
                units.remove_token(file, "Requires=", "repovecd.service");
            }
            Self::MissingMcpdAfterQdrant => {
                units.remove_token(file, "After=", "qdrant.service");
            }
            Self::MissingMcpdAfterRepovecd => {
                units.remove_token(file, "After=", "repovecd.service");
            }
            _ => panic!("service dependency mutation called for another scenario"),
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
