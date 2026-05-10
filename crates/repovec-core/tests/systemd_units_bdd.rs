//! Behavioural tests for the checked-in repovec systemd unit contract.

use repovec_core::appliance::systemd_units::{
    SystemdUnitError, checked_in_repovec_mcpd_service, checked_in_repovec_target,
    checked_in_repovecd_service, validate_systemd_units,
};
use rstest::fixture;
use rstest_bdd_macros::{given, scenario, then, when};

#[derive(Default)]
struct SystemdWorld {
    target: String,
    repovecd: String,
    mcpd: String,
    validation_result: Option<Result<(), SystemdUnitError>>,
}

#[fixture]
fn systemd_world() -> SystemdWorld {
    let validation_result = None;
    SystemdWorld {
        target: String::new(),
        repovecd: String::new(),
        mcpd: String::new(),
        validation_result,
    }
}

#[given("the checked-in repovec systemd units")]
fn the_checked_in_repovec_systemd_units(systemd_world: &mut SystemdWorld) {
    checked_in_repovec_target().clone_into(&mut systemd_world.target);
    checked_in_repovecd_service().clone_into(&mut systemd_world.repovecd);
    checked_in_repovec_mcpd_service().clone_into(&mut systemd_world.mcpd);
}

#[given("cloudflared is removed from the target wants list")]
fn cloudflared_is_removed_from_the_target_wants_list(systemd_world: &mut SystemdWorld) {
    systemd_world.target = systemd_world.target.replace(" cloudflared.service", "");
}

#[given("the target install binding is removed")]
fn the_target_install_binding_is_removed(systemd_world: &mut SystemdWorld) {
    systemd_world.target = systemd_world.target.replace("WantedBy=multi-user.target\n", "");
}

#[given("a semicolon comment is added to the target")]
fn a_semicolon_comment_is_added_to_the_target(systemd_world: &mut SystemdWorld) {
    systemd_world.target =
        systemd_world.target.replace("[Unit]\n", "[Unit]\n; systemd accepts semicolon comments\n");
}

#[given("the repovecd Qdrant ordering is removed")]
fn the_repovecd_qdrant_ordering_is_removed(systemd_world: &mut SystemdWorld) {
    systemd_world.repovecd = systemd_world.repovecd.replace("After=qdrant.service\n", "");
}

#[given("the repovec-mcpd repovecd requirement is removed")]
fn the_repovec_mcpd_repovecd_requirement_is_removed(systemd_world: &mut SystemdWorld) {
    systemd_world.mcpd = systemd_world.mcpd.replace(" repovecd.service", "");
}

#[given("repovecd requires qdrant.container instead of qdrant.service")]
fn repovecd_requires_qdrant_container_instead_of_qdrant_service(systemd_world: &mut SystemdWorld) {
    systemd_world.repovecd = systemd_world.repovecd.replace("qdrant.service", "qdrant.container");
}

#[when("the systemd units are validated")]
fn the_systemd_units_are_validated(systemd_world: &mut SystemdWorld) {
    systemd_world.validation_result = Some(validate_systemd_units(
        &systemd_world.target,
        &systemd_world.repovecd,
        &systemd_world.mcpd,
    ));
}

#[then("the systemd unit set is accepted")]
fn the_systemd_unit_set_is_accepted(systemd_world: &SystemdWorld) {
    let Some(validation_result) = systemd_world.validation_result.as_ref() else {
        panic!("the validation step should have run");
    };

    assert!(validation_result.is_ok());
}

#[then("validation fails because the target does not want cloudflared")]
fn validation_fails_because_the_target_does_not_want_cloudflared(systemd_world: &SystemdWorld) {
    assert_validation_result(
        systemd_world,
        SystemdUnitError::MissingDependency {
            unit: "repovec.target",
            section: "Unit",
            key: "Wants",
            dependency: "cloudflared.service",
        },
    );
}

#[then("validation fails because the target is not wanted by multi-user")]
fn validation_fails_because_the_target_is_not_wanted_by_multi_user(systemd_world: &SystemdWorld) {
    assert_validation_result(
        systemd_world,
        SystemdUnitError::MissingDependency {
            unit: "repovec.target",
            section: "Install",
            key: "WantedBy",
            dependency: "multi-user.target",
        },
    );
}

#[then("validation fails because repovecd does not start after Qdrant")]
fn validation_fails_because_repovecd_does_not_start_after_qdrant(systemd_world: &SystemdWorld) {
    assert_validation_result(
        systemd_world,
        SystemdUnitError::MissingDependency {
            unit: "repovecd.service",
            section: "Unit",
            key: "After",
            dependency: "qdrant.service",
        },
    );
}

#[then("validation fails because repovec-mcpd does not require repovecd")]
fn validation_fails_because_repovec_mcpd_does_not_require_repovecd(systemd_world: &SystemdWorld) {
    assert_validation_result(
        systemd_world,
        SystemdUnitError::MissingDependency {
            unit: "repovec-mcpd.service",
            section: "Unit",
            key: "Requires",
            dependency: "repovecd.service",
        },
    );
}

#[then("validation fails because the Quadlet source name was used")]
fn validation_fails_because_the_quadlet_source_name_was_used(systemd_world: &SystemdWorld) {
    assert_validation_result(
        systemd_world,
        SystemdUnitError::UsesQuadletSourceDependency {
            unit: "repovecd.service",
            section: "Unit",
            key: "Requires",
            dependency: String::from("qdrant.container"),
        },
    );
}

#[scenario(
    path = "tests/features/systemd_units.feature",
    name = "The checked-in unit set satisfies the appliance contract"
)]
fn checked_in_unit_set_satisfies_the_appliance_contract(systemd_world: SystemdWorld) {
    let _ = systemd_world;
}

#[scenario(
    path = "tests/features/systemd_units.feature",
    name = "The target wants every appliance service"
)]
fn target_wants_every_appliance_service(systemd_world: SystemdWorld) { let _ = systemd_world; }

#[scenario(path = "tests/features/systemd_units.feature", name = "The target remains enableable")]
fn target_remains_enableable(systemd_world: SystemdWorld) { let _ = systemd_world; }

#[scenario(path = "tests/features/systemd_units.feature", name = "Semicolon comments are accepted")]
fn semicolon_comments_are_accepted(systemd_world: SystemdWorld) { let _ = systemd_world; }

#[scenario(path = "tests/features/systemd_units.feature", name = "repovecd waits for Qdrant")]
fn repovecd_waits_for_qdrant(systemd_world: SystemdWorld) { let _ = systemd_world; }

#[scenario(
    path = "tests/features/systemd_units.feature",
    name = "repovec-mcpd waits for the control-plane daemon"
)]
fn repovec_mcpd_waits_for_the_control_plane_daemon(systemd_world: SystemdWorld) {
    let _ = systemd_world;
}

#[scenario(
    path = "tests/features/systemd_units.feature",
    name = "The generated Qdrant service name is required"
)]
fn generated_qdrant_service_name_is_required(systemd_world: SystemdWorld) { let _ = systemd_world; }

fn assert_validation_result(systemd_world: &SystemdWorld, expected: SystemdUnitError) {
    let Some(validation_result) = systemd_world.validation_result.as_ref() else {
        panic!("the validation step should have run");
    };

    assert_eq!(validation_result, &Err(expected));
}
