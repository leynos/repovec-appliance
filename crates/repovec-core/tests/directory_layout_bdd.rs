//! Behavioural tests for the checked-in repovec directory-layout contract.

use repovec_core::appliance::directory_layout::{
    DirectoryLayoutError, Mode, checked_in_repovec_sysusers, checked_in_repovec_tmpfiles,
    validate_directory_layout,
};
use rstest::fixture;
use rstest_bdd_macros::{given, scenario, then, when};

#[derive(Default)]
struct DirectoryLayoutWorld {
    tmpfiles: String,
    sysusers: String,
    validation_result: Option<Result<(), DirectoryLayoutError>>,
}

#[fixture]
fn directory_layout_world() -> DirectoryLayoutWorld {
    let validation_result = None;
    DirectoryLayoutWorld { tmpfiles: String::new(), sysusers: String::new(), validation_result }
}

#[given("the checked-in repovec layout assets")]
fn the_checked_in_repovec_layout_assets(directory_layout_world: &mut DirectoryLayoutWorld) {
    checked_in_repovec_tmpfiles().clone_into(&mut directory_layout_world.tmpfiles);
    checked_in_repovec_sysusers().clone_into(&mut directory_layout_world.sysusers);
}

#[given("the worktrees directory entry is removed from the tmpfiles asset")]
fn the_worktrees_directory_entry_is_removed_from_the_tmpfiles_asset(
    directory_layout_world: &mut DirectoryLayoutWorld,
) {
    directory_layout_world.tmpfiles = directory_layout_world
        .tmpfiles
        .replace("d /var/lib/repovec/worktrees 0700 repovec repovec -\n", "");
}

#[given("an unexpected directory entry is added to the tmpfiles asset")]
fn an_unexpected_directory_entry_is_added_to_the_tmpfiles_asset(
    directory_layout_world: &mut DirectoryLayoutWorld,
) {
    directory_layout_world.tmpfiles.push_str("d /var/lib/repovec/cache 0700 repovec repovec -\n");
}

#[given("the worktrees directory mode is widened to 0750")]
fn the_worktrees_directory_mode_is_widened_to_0750(
    directory_layout_world: &mut DirectoryLayoutWorld,
) {
    directory_layout_world.tmpfiles = directory_layout_world.tmpfiles.replace(
        "d /var/lib/repovec/worktrees 0700 repovec repovec -",
        "d /var/lib/repovec/worktrees 0750 repovec repovec -",
    );
}

#[given("the git-mirrors directory owner is changed to root")]
fn the_git_mirrors_directory_owner_is_changed_to_root(
    directory_layout_world: &mut DirectoryLayoutWorld,
) {
    directory_layout_world.tmpfiles = directory_layout_world.tmpfiles.replace(
        "d /var/lib/repovec/git-mirrors 0700 repovec repovec -",
        "d /var/lib/repovec/git-mirrors 0700 root repovec -",
    );
}

#[given("the grepai directory group is changed to wheel")]
fn the_grepai_directory_group_is_changed_to_wheel(
    directory_layout_world: &mut DirectoryLayoutWorld,
) {
    directory_layout_world.tmpfiles = directory_layout_world.tmpfiles.replace(
        "d /var/lib/repovec/.grepai 0700 repovec repovec -",
        "d /var/lib/repovec/.grepai 0700 repovec wheel -",
    );
}

#[given("the worktrees directory mode is replaced with the default token")]
fn the_worktrees_directory_mode_is_replaced_with_the_default_token(
    directory_layout_world: &mut DirectoryLayoutWorld,
) {
    directory_layout_world.tmpfiles = directory_layout_world.tmpfiles.replace(
        "d /var/lib/repovec/worktrees 0700 repovec repovec -",
        "d /var/lib/repovec/worktrees - repovec repovec -",
    );
}

#[given("an /etc/repovec entry is added to the tmpfiles asset")]
fn an_etc_repovec_entry_is_added_to_the_tmpfiles_asset(
    directory_layout_world: &mut DirectoryLayoutWorld,
) {
    directory_layout_world.tmpfiles.push_str("d /etc/repovec 0750 root repovec -\n");
}

#[given("a malformed line is inserted into the tmpfiles asset")]
fn a_malformed_line_is_inserted_into_the_tmpfiles_asset(
    directory_layout_world: &mut DirectoryLayoutWorld,
) {
    directory_layout_world.tmpfiles.push_str("bogus line with not enough fields\n");
}

#[given("the sysusers home path is changed away from /var/lib/repovec")]
fn the_sysusers_home_path_is_changed_away_from_var_lib_repovec(
    directory_layout_world: &mut DirectoryLayoutWorld,
) {
    directory_layout_world.sysusers = directory_layout_world
        .sysusers
        .replace("/var/lib/repovec /usr/sbin/nologin", "/home/repovec /usr/sbin/nologin");
}

#[given("the sysusers shell is changed to /bin/bash")]
fn the_sysusers_shell_is_changed_to_bin_bash(directory_layout_world: &mut DirectoryLayoutWorld) {
    directory_layout_world.sysusers =
        directory_layout_world.sysusers.replace("/usr/sbin/nologin", "/bin/bash");
}

#[when("the directory-layout assets are validated")]
fn the_directory_layout_assets_are_validated(directory_layout_world: &mut DirectoryLayoutWorld) {
    directory_layout_world.validation_result = Some(validate_directory_layout(
        &directory_layout_world.tmpfiles,
        &directory_layout_world.sysusers,
    ));
}

#[then("the directory-layout asset set is accepted")]
fn the_directory_layout_asset_set_is_accepted(directory_layout_world: &DirectoryLayoutWorld) {
    let Some(validation_result) = directory_layout_world.validation_result.as_ref() else {
        panic!("the validation step should have run");
    };

    assert!(validation_result.is_ok());
}

#[then("validation fails because a required directory entry is missing")]
fn validation_fails_because_a_required_directory_entry_is_missing(
    directory_layout_world: &DirectoryLayoutWorld,
) {
    assert_validation_result(
        directory_layout_world,
        DirectoryLayoutError::MissingDirectoryEntry { path: "/var/lib/repovec/worktrees".into() },
    );
}

#[then("validation fails because an unexpected directory entry is present")]
fn validation_fails_because_an_unexpected_directory_entry_is_present(
    directory_layout_world: &DirectoryLayoutWorld,
) {
    assert_validation_result(
        directory_layout_world,
        DirectoryLayoutError::UnexpectedDirectoryEntry { path: "/var/lib/repovec/cache".into() },
    );
}

#[then("validation fails because the directory mode is incorrect")]
fn validation_fails_because_the_directory_mode_is_incorrect(
    directory_layout_world: &DirectoryLayoutWorld,
) {
    assert_validation_result(
        directory_layout_world,
        DirectoryLayoutError::IncorrectMode {
            path: "/var/lib/repovec/worktrees".into(),
            expected: Mode(0o700),
            actual: Mode(0o750),
        },
    );
}

#[then("validation fails because the directory owner is incorrect")]
fn validation_fails_because_the_directory_owner_is_incorrect(
    directory_layout_world: &DirectoryLayoutWorld,
) {
    assert_validation_result(
        directory_layout_world,
        DirectoryLayoutError::IncorrectOwner {
            path: "/var/lib/repovec/git-mirrors".into(),
            expected: "repovec",
            actual: "root".to_owned(),
        },
    );
}

#[then("validation fails because the directory group is incorrect")]
fn validation_fails_because_the_directory_group_is_incorrect(
    directory_layout_world: &DirectoryLayoutWorld,
) {
    assert_validation_result(
        directory_layout_world,
        DirectoryLayoutError::IncorrectGroup {
            path: "/var/lib/repovec/.grepai".into(),
            expected: "repovec",
            actual: "wheel".to_owned(),
        },
    );
}

#[then("validation fails because a required field is not explicit")]
fn validation_fails_because_a_required_field_is_not_explicit(
    directory_layout_world: &DirectoryLayoutWorld,
) {
    let Some(validation_result) = directory_layout_world.validation_result.as_ref() else {
        panic!("the validation step should have run");
    };

    assert!(matches!(
        validation_result,
        Err(DirectoryLayoutError::NonExplicitField { field: "mode", .. })
    ));
}

#[then("validation fails because the secrets directory has a single authority")]
fn validation_fails_because_the_secrets_directory_has_a_single_authority(
    directory_layout_world: &DirectoryLayoutWorld,
) {
    assert_validation_result(
        directory_layout_world,
        DirectoryLayoutError::ForbiddenSecretsEntry { path: "/etc/repovec".into() },
    );
}

#[then("validation fails because a line is malformed")]
fn validation_fails_because_a_line_is_malformed(directory_layout_world: &DirectoryLayoutWorld) {
    let Some(validation_result) = directory_layout_world.validation_result.as_ref() else {
        panic!("the validation step should have run");
    };

    assert!(matches!(validation_result, Err(DirectoryLayoutError::MalformedLine { .. })));
}

#[then("validation fails because the sysusers home is incorrect")]
fn validation_fails_because_the_sysusers_home_is_incorrect(
    directory_layout_world: &DirectoryLayoutWorld,
) {
    assert_validation_result(
        directory_layout_world,
        DirectoryLayoutError::SysusersIncorrectHome {
            expected: "/var/lib/repovec",
            actual: "/home/repovec".to_owned(),
        },
    );
}

#[then("validation fails because the sysusers shell is incorrect")]
fn validation_fails_because_the_sysusers_shell_is_incorrect(
    directory_layout_world: &DirectoryLayoutWorld,
) {
    assert_validation_result(
        directory_layout_world,
        DirectoryLayoutError::SysusersIncorrectShell {
            expected: "/usr/sbin/nologin",
            actual: "/bin/bash".to_owned(),
        },
    );
}

#[scenario(
    path = "tests/features/directory_layout.feature",
    name = "The checked-in layout assets satisfy the appliance contract"
)]
fn checked_in_layout_assets_satisfy_the_appliance_contract(
    directory_layout_world: DirectoryLayoutWorld,
) {
    assert_scenario_steps_ran(&directory_layout_world);
}

#[scenario(
    path = "tests/features/directory_layout.feature",
    name = "The tmpfiles asset must declare every required data directory"
)]
fn tmpfiles_asset_must_declare_every_required_data_directory(
    directory_layout_world: DirectoryLayoutWorld,
) {
    assert_scenario_steps_ran(&directory_layout_world);
}

#[scenario(
    path = "tests/features/directory_layout.feature",
    name = "The tmpfiles asset must not declare unexpected directories"
)]
fn tmpfiles_asset_must_not_declare_unexpected_directories(
    directory_layout_world: DirectoryLayoutWorld,
) {
    assert_scenario_steps_ran(&directory_layout_world);
}

#[scenario(
    path = "tests/features/directory_layout.feature",
    name = "Data directories must be private to the repovec user"
)]
fn data_directories_must_be_private_to_the_repovec_user(
    directory_layout_world: DirectoryLayoutWorld,
) {
    assert_scenario_steps_ran(&directory_layout_world);
}

#[scenario(
    path = "tests/features/directory_layout.feature",
    name = "Data directories must be owned by the repovec user"
)]
fn data_directories_must_be_owned_by_the_repovec_user(
    directory_layout_world: DirectoryLayoutWorld,
) {
    assert_scenario_steps_ran(&directory_layout_world);
}

#[scenario(
    path = "tests/features/directory_layout.feature",
    name = "A data directory group must be repovec"
)]
fn data_directory_group_must_be_repovec(directory_layout_world: DirectoryLayoutWorld) {
    assert_scenario_steps_ran(&directory_layout_world);
}

#[scenario(
    path = "tests/features/directory_layout.feature",
    name = "Directory entries must set explicit modes and ownership"
)]
fn directory_entries_must_set_explicit_modes_and_ownership(
    directory_layout_world: DirectoryLayoutWorld,
) {
    assert_scenario_steps_ran(&directory_layout_world);
}

#[scenario(
    path = "tests/features/directory_layout.feature",
    name = "The tmpfiles asset must not declare the secrets directory"
)]
fn tmpfiles_asset_must_not_declare_the_secrets_directory(
    directory_layout_world: DirectoryLayoutWorld,
) {
    assert_scenario_steps_ran(&directory_layout_world);
}

#[scenario(
    path = "tests/features/directory_layout.feature",
    name = "A malformed tmpfiles line is rejected"
)]
fn malformed_tmpfiles_line_is_rejected(directory_layout_world: DirectoryLayoutWorld) {
    assert_scenario_steps_ran(&directory_layout_world);
}

#[scenario(
    path = "tests/features/directory_layout.feature",
    name = "The sysusers asset must declare the repovec user home"
)]
fn sysusers_asset_must_declare_the_repovec_user_home(directory_layout_world: DirectoryLayoutWorld) {
    assert_scenario_steps_ran(&directory_layout_world);
}

#[scenario(
    path = "tests/features/directory_layout.feature",
    name = "The sysusers asset must keep the nologin shell"
)]
fn sysusers_asset_must_keep_the_nologin_shell(directory_layout_world: DirectoryLayoutWorld) {
    assert_scenario_steps_ran(&directory_layout_world);
}

fn assert_scenario_steps_ran(directory_layout_world: &DirectoryLayoutWorld) {
    assert!(
        directory_layout_world.validation_result.is_some(),
        "the scenario should execute its validation step"
    );
}

fn assert_validation_result(
    directory_layout_world: &DirectoryLayoutWorld,
    expected: DirectoryLayoutError,
) {
    let Some(validation_result) = directory_layout_world.validation_result.as_ref() else {
        panic!("the validation step should have run");
    };

    assert_eq!(validation_result, &Err(expected));
}
