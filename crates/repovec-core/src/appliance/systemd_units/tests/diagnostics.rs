//! Literal diagnostic expectations for systemd unit validator tests.

use super::ValidationScenario;

pub(super) fn expected_diagnostic(scenario: ValidationScenario) -> &'static str {
    match scenario {
        ValidationScenario::InvalidLine
        | ValidationScenario::MissingTargetUnitSection
        | ValidationScenario::MissingTargetInstallSection
        | ValidationScenario::MissingTargetWantedBy
        | ValidationScenario::MissingTargetWantsQdrant
        | ValidationScenario::TargetUsesQdrantContainer
        | ValidationScenario::MissingTargetWantsRepovecd
        | ValidationScenario::MissingTargetWantsMcpd
        | ValidationScenario::MissingTargetWantsCloudflared => expected_target_diagnostic(scenario),
        ValidationScenario::PropertyBeforeSection
        | ValidationScenario::MissingRepovecdServiceSection
        | ValidationScenario::MissingRepovecdRequiresQdrant
        | ValidationScenario::MissingRepovecdAfterQdrant
        | ValidationScenario::RepovecdUsesQdrantContainerService
        | ValidationScenario::WrongRepovecdExecStart
        | ValidationScenario::RepovecdWrongUser
        | ValidationScenario::RepovecdMissingGroup
        | ValidationScenario::RepovecdWrongWorkingDirectory
        | ValidationScenario::RepovecdMissingEnvironment => expected_repovecd_diagnostic(scenario),
        ValidationScenario::MissingMcpdServiceSection
        | ValidationScenario::MissingMcpdRequiresQdrant
        | ValidationScenario::MissingMcpdRequiresRepovecd
        | ValidationScenario::MissingMcpdAfterQdrant
        | ValidationScenario::MissingMcpdAfterRepovecd
        | ValidationScenario::WrongMcpdExecStart
        | ValidationScenario::McpdWrongUser
        | ValidationScenario::McpdMissingGroup
        | ValidationScenario::McpdWrongWorkingDirectory
        | ValidationScenario::McpdMissingEnvironment => expected_mcpd_diagnostic(scenario),
        ValidationScenario::MissingGrepaiTemplateInstallSection
        | ValidationScenario::MissingGrepaiTemplateRequiresQdrant
        | ValidationScenario::MissingGrepaiTemplateRequiresRepovecd
        | ValidationScenario::MissingGrepaiTemplateAfterQdrant
        | ValidationScenario::MissingGrepaiTemplateAfterRepovecd
        | ValidationScenario::GrepaiTemplateUsesQdrantContainer
        | ValidationScenario::MissingGrepaiTemplatePartOfTarget
        | ValidationScenario::MissingGrepaiTemplateWantedByTarget
        | ValidationScenario::WrongGrepaiTemplateType
        | ValidationScenario::WrongGrepaiTemplateExecStart
        | ValidationScenario::GrepaiTemplateWrongUser
        | ValidationScenario::GrepaiTemplateMissingGroup
        | ValidationScenario::GrepaiTemplateWrongWorkingDirectory
        | ValidationScenario::GrepaiTemplateMissingEnvironment
        | ValidationScenario::GrepaiTemplateWrongRestartPolicy
        | ValidationScenario::GrepaiTemplateWrongRestartDelay
        | ValidationScenario::GrepaiTemplateLogsStdoutToFile
        | ValidationScenario::GrepaiTemplateLogsStderrToFile => {
            expected_grepai_template_diagnostic(scenario)
        }
    }
}

fn expected_target_diagnostic(scenario: ValidationScenario) -> &'static str {
    match scenario {
        ValidationScenario::InvalidLine => "invalid systemd line in repovec.target at 2: not valid",
        ValidationScenario::MissingTargetUnitSection => "repovec.target is missing [Unit]",
        ValidationScenario::MissingTargetInstallSection => "repovec.target is missing [Install]",
        ValidationScenario::MissingTargetWantedBy => {
            "repovec.target is missing WantedBy=multi-user.target in [Install]"
        }
        ValidationScenario::MissingTargetWantsQdrant => {
            "repovec.target is missing Wants=qdrant.service in [Unit]"
        }
        ValidationScenario::TargetUsesQdrantContainer => concat!(
            "repovec.target must depend on qdrant.service, not qdrant.container, ",
            "in [Unit] Wants",
        ),
        ValidationScenario::MissingTargetWantsRepovecd => {
            "repovec.target is missing Wants=repovecd.service in [Unit]"
        }
        ValidationScenario::MissingTargetWantsMcpd => {
            "repovec.target is missing Wants=repovec-mcpd.service in [Unit]"
        }
        ValidationScenario::MissingTargetWantsCloudflared => {
            "repovec.target is missing Wants=cloudflared.service in [Unit]"
        }
        _ => panic!("target diagnostic called for non-target scenario"),
    }
}

fn expected_repovecd_diagnostic(scenario: ValidationScenario) -> &'static str {
    match scenario {
        ValidationScenario::PropertyBeforeSection => concat!(
            "systemd property before section in repovecd.service on line 1: ",
            "Requires=qdrant.service",
        ),
        ValidationScenario::MissingRepovecdServiceSection => {
            "repovecd.service is missing [Service]"
        }
        ValidationScenario::MissingRepovecdRequiresQdrant => {
            "repovecd.service is missing Requires=qdrant.service in [Unit]"
        }
        ValidationScenario::MissingRepovecdAfterQdrant => {
            "repovecd.service is missing After=qdrant.service in [Unit]"
        }
        ValidationScenario::RepovecdUsesQdrantContainerService => concat!(
            "repovecd.service must depend on qdrant.service, not ",
            "qdrant.container.service, in [Unit] Requires",
        ),
        ValidationScenario::WrongRepovecdExecStart => {
            "repovecd.service must use ExecStart=/usr/bin/repovecd: /usr/bin/otherd"
        }
        ValidationScenario::RepovecdWrongUser => {
            "repovecd.service must have User=repovec in [Service]: root"
        }
        ValidationScenario::RepovecdMissingGroup => {
            "repovecd.service must have Group=repovec in [Service]: "
        }
        ValidationScenario::RepovecdWrongWorkingDirectory => concat!(
            "repovecd.service must have WorkingDirectory=/var/lib/repovec ",
            "in [Service]: /tmp",
        ),
        ValidationScenario::RepovecdMissingEnvironment => concat!(
            "repovecd.service must have Environment=HOME=/var/lib/repovec ",
            "in [Service]: ",
        ),
        _ => panic!("repovecd diagnostic called for non-repovecd scenario"),
    }
}

fn expected_mcpd_diagnostic(scenario: ValidationScenario) -> &'static str {
    match scenario {
        ValidationScenario::MissingMcpdServiceSection => {
            "repovec-mcpd.service is missing [Service]"
        }
        ValidationScenario::MissingMcpdRequiresQdrant => {
            "repovec-mcpd.service is missing Requires=qdrant.service in [Unit]"
        }
        ValidationScenario::MissingMcpdRequiresRepovecd => {
            "repovec-mcpd.service is missing Requires=repovecd.service in [Unit]"
        }
        ValidationScenario::MissingMcpdAfterQdrant => {
            "repovec-mcpd.service is missing After=qdrant.service in [Unit]"
        }
        ValidationScenario::MissingMcpdAfterRepovecd => {
            "repovec-mcpd.service is missing After=repovecd.service in [Unit]"
        }
        ValidationScenario::WrongMcpdExecStart => concat!(
            "repovec-mcpd.service must use ExecStart=/usr/bin/repovec-mcpd: ",
            "/usr/bin/other-mcpd",
        ),
        ValidationScenario::McpdWrongUser => {
            "repovec-mcpd.service must have User=repovec in [Service]: root"
        }
        ValidationScenario::McpdMissingGroup => {
            "repovec-mcpd.service must have Group=repovec in [Service]: "
        }
        ValidationScenario::McpdWrongWorkingDirectory => concat!(
            "repovec-mcpd.service must have WorkingDirectory=/var/lib/repovec ",
            "in [Service]: /tmp",
        ),
        ValidationScenario::McpdMissingEnvironment => concat!(
            "repovec-mcpd.service must have Environment=HOME=/var/lib/repovec ",
            "in [Service]: ",
        ),
        _ => panic!("repovec-mcpd diagnostic called for non-mcpd scenario"),
    }
}

fn expected_grepai_template_diagnostic(scenario: ValidationScenario) -> &'static str {
    match scenario {
        ValidationScenario::MissingGrepaiTemplateInstallSection => {
            "repovec-grepai@.service is missing [Install]"
        }
        ValidationScenario::MissingGrepaiTemplateRequiresQdrant => {
            "repovec-grepai@.service is missing Requires=qdrant.service in [Unit]"
        }
        ValidationScenario::MissingGrepaiTemplateRequiresRepovecd => {
            "repovec-grepai@.service is missing Requires=repovecd.service in [Unit]"
        }
        ValidationScenario::MissingGrepaiTemplateAfterQdrant => {
            "repovec-grepai@.service is missing After=qdrant.service in [Unit]"
        }
        ValidationScenario::MissingGrepaiTemplateAfterRepovecd => {
            "repovec-grepai@.service is missing After=repovecd.service in [Unit]"
        }
        ValidationScenario::GrepaiTemplateUsesQdrantContainer => concat!(
            "repovec-grepai@.service must depend on qdrant.service, not ",
            "qdrant.container, in [Unit] Requires",
        ),
        ValidationScenario::MissingGrepaiTemplatePartOfTarget => {
            "repovec-grepai@.service is missing PartOf=repovec.target in [Unit]"
        }
        ValidationScenario::MissingGrepaiTemplateWantedByTarget => {
            "repovec-grepai@.service is missing WantedBy=repovec.target in [Install]"
        }
        ValidationScenario::WrongGrepaiTemplateType => {
            "repovec-grepai@.service must have Type=exec in [Service]: simple"
        }
        ValidationScenario::WrongGrepaiTemplateExecStart => concat!(
            "repovec-grepai@.service must use ExecStart=/usr/bin/grepai watch: ",
            "/usr/bin/grepai",
        ),
        ValidationScenario::GrepaiTemplateWrongUser => {
            "repovec-grepai@.service must have User=repovec in [Service]: root"
        }
        ValidationScenario::GrepaiTemplateMissingGroup => {
            "repovec-grepai@.service must have Group=repovec in [Service]: "
        }
        ValidationScenario::GrepaiTemplateWrongWorkingDirectory => concat!(
            "repovec-grepai@.service must have WorkingDirectory=",
            "/var/lib/repovec/worktrees/%I in [Service]: /var/lib/repovec",
        ),
        ValidationScenario::GrepaiTemplateMissingEnvironment => concat!(
            "repovec-grepai@.service must have Environment=HOME=/var/lib/repovec ",
            "in [Service]: ",
        ),
        ValidationScenario::GrepaiTemplateWrongRestartPolicy => {
            "repovec-grepai@.service must have Restart=on-failure in [Service]: always"
        }
        ValidationScenario::GrepaiTemplateWrongRestartDelay => {
            "repovec-grepai@.service must have RestartSec=5s in [Service]: 0"
        }
        ValidationScenario::GrepaiTemplateLogsStdoutToFile => concat!(
            "repovec-grepai@.service must have StandardOutput=journal in [Service]: ",
            "file:/var/log/repovec/grepai.log",
        ),
        ValidationScenario::GrepaiTemplateLogsStderrToFile => concat!(
            "repovec-grepai@.service must have StandardError=journal in [Service]: ",
            "file:/var/log/repovec/grepai.err",
        ),
        _ => panic!("grepai template diagnostic called for non-template scenario"),
    }
}
