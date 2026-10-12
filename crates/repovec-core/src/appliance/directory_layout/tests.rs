//! Contract tests for `validate_directory_layout`: deterministic mutations of
//! the checked-in assets against the per-directory appliance contract.

use camino::Utf8PathBuf;
use rstest::rstest;

use super::tests_support::{Mutation, assert_mutation, checked_in};
use crate::appliance::directory_layout::{
    DirectoryLayoutError, Mode,
    parser::{ModeParseError, parse_mode},
    validate_directory_layout,
};

#[test]
fn checked_in_assets_satisfy_the_directory_contract() {
    let (tmpfiles, sysusers) = checked_in();
    validate_directory_layout(&tmpfiles, &sysusers)
        .expect("the checked-in assets must satisfy the directory contract");
}

/// Every mutation below is a distinct semantic failure of the contract. Keeping
/// them as `#[case]` rows (rather than a loop) makes each one addressable by
/// name, so a failure report points straight at the violated rule.
#[rstest]
#[case::incorrect_mode(
    Mutation::tmpfiles(
        "d /var/lib/repovec/worktrees 0700 repovec repovec -",
        "d /var/lib/repovec/worktrees 0750 repovec repovec -",
    ),
    DirectoryLayoutError::IncorrectMode {
        path: Utf8PathBuf::from("/var/lib/repovec/worktrees"),
        expected: Mode(0o700),
        actual: Mode(0o750),
    },
)]
#[case::mode_placeholder(
    Mutation::tmpfiles(
        "d /var/lib/repovec/worktrees 0700 repovec repovec -",
        "d /var/lib/repovec/worktrees - repovec repovec -",
    ),
    DirectoryLayoutError::NonExplicitField {
        asset: "tmpfiles.d/repovec.conf",
        line_number: 7,
        field: "mode",
    },
)]
#[case::mode_not_octal(
    Mutation::tmpfiles(
        "d /var/lib/repovec/worktrees 0700 repovec repovec -",
        "d /var/lib/repovec/worktrees 0999 repovec repovec -",
    ),
    DirectoryLayoutError::InvalidField {
        asset: "tmpfiles.d/repovec.conf",
        line_number: 7,
        field: "mode",
        value: "0999".to_owned(),
    },
)]
#[case::incorrect_owner(
    Mutation::tmpfiles(
        "d /var/lib/repovec/git-mirrors 0700 repovec repovec -",
        "d /var/lib/repovec/git-mirrors 0700 root repovec -",
    ),
    DirectoryLayoutError::IncorrectOwner {
        path: Utf8PathBuf::from("/var/lib/repovec/git-mirrors"),
        expected: "repovec",
        actual: "root".to_owned(),
    },
)]
#[case::incorrect_group(
    Mutation::tmpfiles(
        "d /var/lib/repovec/.grepai 0700 repovec repovec -",
        "d /var/lib/repovec/.grepai 0700 repovec wheel -",
    ),
    DirectoryLayoutError::IncorrectGroup {
        path: Utf8PathBuf::from("/var/lib/repovec/.grepai"),
        expected: "repovec",
        actual: "wheel".to_owned(),
    },
)]
#[case::missing_directory(
    Mutation::tmpfiles("d /var/lib/repovec/worktrees 0700 repovec repovec -\n", ""),
    DirectoryLayoutError::MissingDirectoryEntry {
        path: Utf8PathBuf::from("/var/lib/repovec/worktrees"),
    },
)]
#[case::unexpected_directory(
    Mutation::tmpfiles("\n", "\nd /var/lib/repovec/cache 0700 repovec repovec -\n"),
    DirectoryLayoutError::UnexpectedDirectoryEntry {
        path: Utf8PathBuf::from("/var/lib/repovec/cache"),
    },
)]
#[case::forbidden_secrets_entry(
    Mutation::tmpfiles("\n", "\nd /etc/repovec 0750 root repovec -\n"),
    DirectoryLayoutError::ForbiddenSecretsEntry { path: Utf8PathBuf::from("/etc/repovec") },
)]
#[case::forbidden_secrets_nested(
    Mutation::tmpfiles("\n", "\nd /etc/repovec/keys 0750 root repovec -\n"),
    DirectoryLayoutError::ForbiddenSecretsEntry { path: Utf8PathBuf::from("/etc/repovec/keys") },
)]
#[case::sibling_path_is_not_the_secrets_root(
    // `/etc/repovec-backup` shares a string prefix with `/etc/repovec` but is a
    // different directory, so it is an unexpected entry, not a second authority
    // for the secrets file.
    Mutation::tmpfiles("\n", "\nd /etc/repovec-backup 0750 root repovec -\n"),
    DirectoryLayoutError::UnexpectedDirectoryEntry {
        path: Utf8PathBuf::from("/etc/repovec-backup"),
    },
)]
#[case::sysusers_incorrect_home(
    Mutation::sysusers(
        "/var/lib/repovec /usr/sbin/nologin",
        "/home/repovec /usr/sbin/nologin",
    ),
    DirectoryLayoutError::SysusersIncorrectHome {
        expected: "/var/lib/repovec",
        actual: "/home/repovec".to_owned(),
    },
)]
#[case::sysusers_incorrect_shell(
    Mutation::sysusers("/usr/sbin/nologin", "/bin/bash"),
    DirectoryLayoutError::SysusersIncorrectShell {
        expected: "/usr/sbin/nologin",
        actual: "/bin/bash".to_owned(),
    },
)]
#[case::sysusers_pinned_uid(
    Mutation::sysusers("u repovec - ", "u repovec 1000 "),
    DirectoryLayoutError::SysusersPinnedUid { actual: "1000".to_owned() },
)]
fn directory_mutations_fail_with_typed_errors(
    #[case] mutation: Mutation,
    #[case] expected: DirectoryLayoutError,
) {
    assert_mutation(mutation, expected);
}

/// A foreign account declared by the same asset is not a failure: the validator
/// is looking for the repovec declaration, and other accounts are simply not it.
#[test]
fn a_foreign_sysusers_line_does_not_count_as_the_repovec_user() {
    let mutation = Mutation::sysusers(
        "u repovec - \"repovec appliance service user\" /var/lib/repovec /usr/sbin/nologin",
        "u other - \"other service user\" /var/lib/other /usr/sbin/nologin",
    );

    assert_mutation(mutation, DirectoryLayoutError::SysusersMissingUser);
}

/// Octal-mode normalisation: `0750` and `750` denote the same permission set.
#[rstest]
#[case("0750", Mode(0o750))]
#[case("750", Mode(0o750))]
#[case("0000", Mode(0o000))]
#[case("0700", Mode(0o700))]
fn mode_parsing_normalizes_octal_spellings(#[case] text: &str, #[case] expected: Mode) {
    assert_eq!(parse_mode(text), Ok(expected));
}

/// The `-` placeholder is reported separately from an unparsable mode so the
/// validator can tell the operator the mode is missing rather than invalid.
#[rstest]
#[case("-", ModeParseError::Placeholder)]
#[case("0999", ModeParseError::Invalid)]
#[case("", ModeParseError::Invalid)]
#[case("rwx", ModeParseError::Invalid)]
fn mode_parsing_rejects_placeholder_and_invalid_spellings(
    #[case] text: &str,
    #[case] expected: ModeParseError,
) {
    assert_eq!(parse_mode(text), Err(expected));
}
