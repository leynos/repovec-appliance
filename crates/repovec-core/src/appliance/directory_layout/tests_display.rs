//! Operator-facing `Display` snapshots for directory-layout validation errors.

use camino::Utf8PathBuf;
use insta::assert_snapshot;

use super::tests_support::{Mutation, validate_mutation};
use crate::appliance::directory_layout::{DirectoryLayoutError, Mode};

/// Operator-facing `Display` messages are captured as inline snapshots from
/// real `validate_directory_layout` failures so the human-readable contract
/// text cannot drift unnoticed. Every variant is covered by its own snapshot
/// test (per-variant inline snapshots need distinct source locations).
#[test]
fn incorrect_mode_display_is_snapshotted() {
    let mutation = Mutation::tmpfiles(
        "d /var/lib/repovec/worktrees 0700 repovec repovec -",
        "d /var/lib/repovec/worktrees 0750 repovec repovec -",
    );
    let expected = DirectoryLayoutError::IncorrectMode {
        path: Utf8PathBuf::from("/var/lib/repovec/worktrees"),
        expected: Mode(0o700),
        actual: Mode(0o750),
    };
    let err = validate_mutation(mutation);
    assert_eq!(err, expected);
    assert_snapshot!(err.to_string(), @"/var/lib/repovec/worktrees must have mode 0700: 0750");
}

#[test]
fn non_explicit_field_display_is_snapshotted() {
    let mutation = Mutation::tmpfiles(
        "d /var/lib/repovec/worktrees 0700 repovec repovec -",
        "d /var/lib/repovec/worktrees - repovec repovec -",
    );
    let expected = DirectoryLayoutError::NonExplicitField {
        asset: "tmpfiles.d/repovec.conf",
        line_number: 7,
        field: "mode",
    };
    let err = validate_mutation(mutation);
    assert_eq!(err, expected);
    assert_snapshot!(err.to_string(), @"tmpfiles.d/repovec.conf must set mode explicitly on line 7");
}

#[test]
fn invalid_field_display_is_snapshotted() {
    let mutation = Mutation::tmpfiles(
        "d /var/lib/repovec/worktrees 0700 repovec repovec -",
        "d /var/lib/repovec/worktrees 0999 repovec repovec -",
    );
    let expected = DirectoryLayoutError::InvalidField {
        asset: "tmpfiles.d/repovec.conf",
        line_number: 7,
        field: "mode",
        value: "0999".to_owned(),
    };
    let err = validate_mutation(mutation);
    assert_eq!(err, expected);
    assert_snapshot!(
        err.to_string(),
        @"tmpfiles.d/repovec.conf has an invalid mode on line 7: 0999"
    );
}

#[test]
fn incorrect_owner_display_is_snapshotted() {
    let mutation = Mutation::tmpfiles(
        "d /var/lib/repovec/git-mirrors 0700 repovec repovec -",
        "d /var/lib/repovec/git-mirrors 0700 root repovec -",
    );
    let expected = DirectoryLayoutError::IncorrectOwner {
        path: Utf8PathBuf::from("/var/lib/repovec/git-mirrors"),
        expected: "repovec",
        actual: "root".to_owned(),
    };
    let err = validate_mutation(mutation);
    assert_eq!(err, expected);
    assert_snapshot!(err.to_string(), @"/var/lib/repovec/git-mirrors must be owned by repovec: root");
}

#[test]
fn incorrect_group_display_is_snapshotted() {
    let mutation = Mutation::tmpfiles(
        "d /var/lib/repovec/.grepai 0700 repovec repovec -",
        "d /var/lib/repovec/.grepai 0700 repovec wheel -",
    );
    let expected = DirectoryLayoutError::IncorrectGroup {
        path: Utf8PathBuf::from("/var/lib/repovec/.grepai"),
        expected: "repovec",
        actual: "wheel".to_owned(),
    };
    let err = validate_mutation(mutation);
    assert_eq!(err, expected);
    assert_snapshot!(err.to_string(), @"/var/lib/repovec/.grepai must be in group repovec: wheel");
}

#[test]
fn missing_directory_display_is_snapshotted() {
    let mutation = Mutation::tmpfiles("d /var/lib/repovec/worktrees 0700 repovec repovec -\n", "");
    let expected = DirectoryLayoutError::MissingDirectoryEntry {
        path: Utf8PathBuf::from("/var/lib/repovec/worktrees"),
    };
    let err = validate_mutation(mutation);
    assert_eq!(err, expected);
    assert_snapshot!(err.to_string(), @"missing directory entry for /var/lib/repovec/worktrees");
}

#[test]
fn unexpected_directory_display_is_snapshotted() {
    let mutation = Mutation::tmpfiles("\n", "\nd /var/lib/repovec/cache 0700 repovec repovec -\n");
    let expected = DirectoryLayoutError::UnexpectedDirectoryEntry {
        path: Utf8PathBuf::from("/var/lib/repovec/cache"),
    };
    let err = validate_mutation(mutation);
    assert_eq!(err, expected);
    assert_snapshot!(err.to_string(), @"unexpected directory entry for /var/lib/repovec/cache");
}

#[test]
fn forbidden_secrets_display_is_snapshotted() {
    let mutation = Mutation::tmpfiles("\n", "\nd /etc/repovec 0750 root repovec -\n");
    let expected =
        DirectoryLayoutError::ForbiddenSecretsEntry { path: Utf8PathBuf::from("/etc/repovec") };
    let err = validate_mutation(mutation);
    assert_eq!(err, expected);
    assert_snapshot!(err.to_string(), @"/etc/repovec must not be declared by the tmpfiles.d/repovec.conf asset");
}

#[test]
fn malformed_line_display_is_snapshotted() {
    let mutation = Mutation::tmpfiles("\n", "\nx /var/lib/repovec/bogus 0700 repovec repovec -\n");
    let expected = DirectoryLayoutError::MalformedLine {
        asset: "tmpfiles.d/repovec.conf",
        line_number: 2,
        line: "x /var/lib/repovec/bogus 0700 repovec repovec -".to_owned(),
    };
    let err = validate_mutation(mutation);
    assert_eq!(err, expected);
    assert_snapshot!(
        err.to_string(),
        @"malformed line in tmpfiles.d/repovec.conf at 2: x /var/lib/repovec/bogus 0700 repovec repovec -"
    );
}

#[test]
fn sysusers_missing_user_display_is_snapshotted() {
    let mutation = Mutation::sysusers(
        "u repovec - \"repovec appliance service user\" /var/lib/repovec /usr/sbin/nologin",
        "u other - \"repovec appliance service user\" /var/lib/repovec /usr/sbin/nologin",
    );
    let err = validate_mutation(mutation);
    assert_eq!(err, DirectoryLayoutError::SysusersMissingUser);
    assert_snapshot!(err.to_string(), @"the sysusers.d/repovec.conf asset must declare the repovec user");
}

#[test]
fn sysusers_pinned_uid_display_is_snapshotted() {
    let mutation = Mutation::sysusers("u repovec - ", "u repovec 1000 ");
    let err = validate_mutation(mutation);
    assert_eq!(err, DirectoryLayoutError::SysusersPinnedUid { actual: "1000".to_owned() });
    assert_snapshot!(
        err.to_string(),
        @"the repovec user uid must be allocated by systemd-sysusers, not pinned: 1000"
    );
}

#[test]
fn sysusers_incorrect_home_display_is_snapshotted() {
    let mutation =
        Mutation::sysusers("/var/lib/repovec /usr/sbin/nologin", "/home/repovec /usr/sbin/nologin");
    let expected = DirectoryLayoutError::SysusersIncorrectHome {
        expected: "/var/lib/repovec",
        actual: "/home/repovec".to_owned(),
    };
    let err = validate_mutation(mutation);
    assert_eq!(err, expected);
    assert_snapshot!(err.to_string(), @"the repovec user home must be /var/lib/repovec: /home/repovec");
}

#[test]
fn sysusers_incorrect_shell_display_is_snapshotted() {
    let mutation = Mutation::sysusers("/usr/sbin/nologin", "/bin/bash");
    let expected = DirectoryLayoutError::SysusersIncorrectShell {
        expected: "/usr/sbin/nologin",
        actual: "/bin/bash".to_owned(),
    };
    let err = validate_mutation(mutation);
    assert_eq!(err, expected);
    assert_snapshot!(err.to_string(), @"the repovec user shell must be /usr/sbin/nologin: /bin/bash");
}
