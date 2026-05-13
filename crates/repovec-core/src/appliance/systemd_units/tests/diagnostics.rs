//! Literal diagnostic expectations for systemd unit validator tests.

use super::ValidationScenario;

pub(super) fn expected_diagnostic(scenario: ValidationScenario) -> &'static str {
    match scenario {
        ValidationScenario::InvalidLine
        | ValidationScenario::PropertyBeforeSection
        | ValidationScenario::MissingTargetUnitSection
        | ValidationScenario::MissingTargetInstallSection
        | ValidationScenario::MissingTargetWantedBy
        | ValidationScenario::MissingTargetWantsQdrant
        | ValidationScenario::TargetUsesQdrantContainer
        | ValidationScenario::MissingTargetWantsRepovecd
        | ValidationScenario::MissingTargetWantsMcpd
        | ValidationScenario::MissingTargetWantsCloudflared => expected_target_diagnostic(scenario),
        ValidationScenario::MissingRepovecdRequiresQdrant
        | ValidationScenario::MissingRepovecdAfterQdrant
        | ValidationScenario::RepovecdUsesQdrantContainerService
        | ValidationScenario::WrongRepovecdExecStart
        | ValidationScenario::RepovecdWrongUser
        | ValidationScenario::RepovecdMissingGroup => expected_repovecd_diagnostic(scenario),
        ValidationScenario::MissingMcpdRequiresQdrant
        | ValidationScenario::MissingMcpdRequiresRepovecd
        | ValidationScenario::MissingMcpdAfterQdrant
        | ValidationScenario::MissingMcpdAfterRepovecd
        | ValidationScenario::WrongMcpdExecStart
        | ValidationScenario::McpdWrongWorkingDirectory
        | ValidationScenario::McpdMissingEnvironment => expected_mcpd_diagnostic(scenario),
    }
}

fn expected_target_diagnostic(scenario: ValidationScenario) -> &'static str {
    match scenario {
        ValidationScenario::InvalidLine => "invalid systemd line in repovec.target at 2: not valid",
        ValidationScenario::PropertyBeforeSection => concat!(
            "systemd property before section in repovecd.service on line 1: ",
            "Requires=qdrant.service",
        ),
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
        _ => panic!("repovecd diagnostic called for non-repovecd scenario"),
    }
}

fn expected_mcpd_diagnostic(scenario: ValidationScenario) -> &'static str {
    match scenario {
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
