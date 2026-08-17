//! Contract tests for `validate_systemd_units`: deterministic mutations of the
//! shipped systemd units plus committed `insta` diagnostic snapshots.

#[path = "tests/error_builders.rs"]
mod error_builders;
#[path = "tests/expected_errors.rs"]
mod expected_errors;
#[path = "tests/grepai_template_mutations.rs"]
mod grepai_template_mutations;
#[path = "tests/hardening.rs"]
mod hardening;
#[path = "tests/passing.rs"]
mod passing;
#[path = "tests/scenarios.rs"]
mod scenarios;
#[path = "tests/snapshot_labels.rs"]
mod snapshot_labels;
#[path = "tests/unit_set.rs"]
mod unit_set;

use insta::assert_snapshot;
use rstest::rstest;
use scenarios::ValidationScenario;
use unit_set::{UnitSet, checked_in_unit_set};

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
#[case::missing_target_wants_provision(ValidationScenario::MissingTargetWantsProvision)]
#[case::missing_provision_unit_section(ValidationScenario::MissingProvisionUnitSection)]
#[case::missing_provision_service_section(ValidationScenario::MissingProvisionServiceSection)]
#[case::missing_provision_install_section(ValidationScenario::MissingProvisionInstallSection)]
#[case::missing_provision_wanted_by_target(ValidationScenario::MissingProvisionWantedByTarget)]
#[case::missing_provision_after_sysusers(ValidationScenario::MissingProvisionAfterSysusers)]
#[case::missing_provision_before_repovecd(ValidationScenario::MissingProvisionBeforeRepovecd)]
#[case::wrong_provision_sysusers_exec_start(ValidationScenario::WrongProvisionSysusersExecStart)]
#[case::missing_provision_type(ValidationScenario::MissingProvisionType)]
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
#[case::missing_grepai_template_unit_section(ValidationScenario::MissingGrepaiTemplateUnitSection)]
#[case::missing_grepai_template_service_section(
    ValidationScenario::MissingGrepaiTemplateServiceSection
)]
#[case::missing_grepai_template_install_section(
    ValidationScenario::MissingGrepaiTemplateInstallSection
)]
#[case::missing_grepai_template_requires_qdrant(
    ValidationScenario::MissingGrepaiTemplateRequiresQdrant
)]
#[case::missing_grepai_template_requires_repovecd(
    ValidationScenario::MissingGrepaiTemplateRequiresRepovecd
)]
#[case::missing_grepai_template_after_qdrant(ValidationScenario::MissingGrepaiTemplateAfterQdrant)]
#[case::missing_grepai_template_after_repovecd(
    ValidationScenario::MissingGrepaiTemplateAfterRepovecd
)]
#[case::grepai_template_uses_qdrant_container(
    ValidationScenario::GrepaiTemplateUsesQdrantContainer
)]
#[case::missing_grepai_template_part_of_target(
    ValidationScenario::MissingGrepaiTemplatePartOfTarget
)]
#[case::missing_grepai_template_wanted_by_target(
    ValidationScenario::MissingGrepaiTemplateWantedByTarget
)]
#[case::wrong_grepai_template_type(ValidationScenario::WrongGrepaiTemplateType)]
#[case::wrong_grepai_template_exec_start(ValidationScenario::WrongGrepaiTemplateExecStart)]
#[case::grepai_template_wrong_user(ValidationScenario::GrepaiTemplateWrongUser)]
#[case::grepai_template_missing_group(ValidationScenario::GrepaiTemplateMissingGroup)]
#[case::grepai_template_wrong_working_directory(
    ValidationScenario::GrepaiTemplateWrongWorkingDirectory
)]
#[case::grepai_template_missing_environment(ValidationScenario::GrepaiTemplateMissingEnvironment)]
#[case::grepai_template_wrong_restart_policy(ValidationScenario::GrepaiTemplateWrongRestartPolicy)]
#[case::grepai_template_wrong_restart_delay(ValidationScenario::GrepaiTemplateWrongRestartDelay)]
#[case::grepai_template_logs_stdout_to_file(ValidationScenario::GrepaiTemplateLogsStdoutToFile)]
#[case::grepai_template_logs_stderr_to_file(ValidationScenario::GrepaiTemplateLogsStderrToFile)]
fn validated_systemd_unit_violations_match_expected_variant_and_diagnostic_snapshots(
    checked_in_unit_set: UnitSet,
    #[case] scenario: ValidationScenario,
) {
    let units = scenario.mutate(checked_in_unit_set);
    let Err(err) = units.validate() else {
        panic!("expected {scenario:?} validation to fail");
    };

    assert_eq!(err, scenario.expected_error());
    assert_snapshot!(scenario.snapshot_label(), err.to_string());
}
