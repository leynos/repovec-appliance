//! Contract tests for `validate_directory_layout`: deterministic mutations of
//! the checked-in assets against the per-directory appliance contract.

use camino::Utf8PathBuf;
use insta::assert_snapshot;
use rstest::rstest;

use crate::appliance::directory_layout::{
    DirectoryLayoutError, Mode, checked_in_repovec_sysusers, checked_in_repovec_tmpfiles,
    validate_directory_layout,
};

/// A mutation that rewrites the checked-in asset pair into a violating state.
///
/// The mutation is expressed as a pair of string replacements so the test stays
/// readable; each case mutates exactly one of the two assets.
struct Mutation {
    tmpfiles_from: Option<&'static str>,
    tmpfiles_to: &'static str,
    sysusers_from: Option<&'static str>,
    sysusers_to: &'static str,
}

impl Mutation {
    const fn tmpfiles(from: &'static str, to: &'static str) -> Self {
        Self { tmpfiles_from: Some(from), tmpfiles_to: to, sysusers_from: None, sysusers_to: "" }
    }

    const fn sysusers(from: &'static str, to: &'static str) -> Self {
        Self { tmpfiles_from: None, tmpfiles_to: "", sysusers_from: Some(from), sysusers_to: to }
    }

    fn apply(&self, tmpfiles: &mut String, sysusers: &mut String) {
        if let Some(from) = self.tmpfiles_from {
            *tmpfiles = tmpfiles.replacen(from, self.tmpfiles_to, 1);
        }
        if let Some(from) = self.sysusers_from {
            *sysusers = sysusers.replacen(from, self.sysusers_to, 1);
        }
    }
}

fn checked_in() -> (String, String) {
    (checked_in_repovec_tmpfiles().to_owned(), checked_in_repovec_sysusers().to_owned())
}

fn assert_mutation(mutation: &Mutation, expected: DirectoryLayoutError) {
    let (mut tmpfiles, mut sysusers) = checked_in();
    mutation.apply(&mut tmpfiles, &mut sysusers);

    let result = validate_directory_layout(&tmpfiles, &sysusers);

    assert_eq!(result, Err(expected));
}

#[test]
fn checked_in_assets_satisfy_the_directory_contract() {
    let (tmpfiles, sysusers) = checked_in();
    validate_directory_layout(&tmpfiles, &sysusers)
        .expect("the checked-in assets must satisfy the directory contract");
}

#[rstest]
fn directory_mutations_fail_with_typed_errors() {
    let cases = [
        (
            Mutation::tmpfiles(
                "d /var/lib/repovec/worktrees 0700 repovec repovec -",
                "d /var/lib/repovec/worktrees 0750 repovec repovec -",
            ),
            DirectoryLayoutError::IncorrectMode {
                path: Utf8PathBuf::from("/var/lib/repovec/worktrees"),
                expected: Mode(0o700),
                actual: Mode(0o750),
            },
        ),
        (
            Mutation::tmpfiles(
                "d /var/lib/repovec/worktrees 0700 repovec repovec -",
                "d /var/lib/repovec/worktrees - repovec repovec -",
            ),
            DirectoryLayoutError::NonExplicitField {
                asset: "tmpfiles.d/repovec.conf",
                line_number: 7,
                field: "mode",
            },
        ),
        (
            Mutation::tmpfiles(
                "d /var/lib/repovec/git-mirrors 0700 repovec repovec -",
                "d /var/lib/repovec/git-mirrors 0700 root repovec -",
            ),
            DirectoryLayoutError::IncorrectOwner {
                path: Utf8PathBuf::from("/var/lib/repovec/git-mirrors"),
                expected: "repovec",
                actual: "root".to_owned(),
            },
        ),
        (
            Mutation::tmpfiles(
                "d /var/lib/repovec/.grepai 0700 repovec repovec -",
                "d /var/lib/repovec/.grepai 0700 repovec wheel -",
            ),
            DirectoryLayoutError::IncorrectGroup {
                path: Utf8PathBuf::from("/var/lib/repovec/.grepai"),
                expected: "repovec",
                actual: "wheel".to_owned(),
            },
        ),
        (
            Mutation::tmpfiles("d /var/lib/repovec/worktrees 0700 repovec repovec -\n", ""),
            DirectoryLayoutError::MissingDirectoryEntry {
                path: Utf8PathBuf::from("/var/lib/repovec/worktrees"),
            },
        ),
        (
            Mutation::tmpfiles("\n", "\nd /var/lib/repovec/cache 0700 repovec repovec -\n"),
            DirectoryLayoutError::UnexpectedDirectoryEntry {
                path: Utf8PathBuf::from("/var/lib/repovec/cache"),
            },
        ),
        (
            Mutation::tmpfiles("\n", "\nd /etc/repovec 0750 root repovec -\n"),
            DirectoryLayoutError::ForbiddenSecretsEntry { path: Utf8PathBuf::from("/etc/repovec") },
        ),
        (
            Mutation::sysusers(
                "/var/lib/repovec /usr/sbin/nologin",
                "/home/repovec /usr/sbin/nologin",
            ),
            DirectoryLayoutError::SysusersIncorrectHome {
                expected: "/var/lib/repovec",
                actual: "/home/repovec".to_owned(),
            },
        ),
        (
            Mutation::sysusers("/usr/sbin/nologin", "/bin/bash"),
            DirectoryLayoutError::SysusersIncorrectShell {
                expected: "/usr/sbin/nologin",
                actual: "/bin/bash".to_owned(),
            },
        ),
    ];

    for (mutation, expected) in cases {
        assert_mutation(&mutation, expected);
    }
}

/// Operator-facing `Display` messages are captured as inline snapshots from
/// real `validate_directory_layout` failures so the human-readable contract
/// text cannot drift unnoticed. Every variant is covered by its own snapshot
/// test (per-variant inline snapshots need distinct source locations).
fn validate_mutation(mutation: &Mutation) -> DirectoryLayoutError {
    let (mut tmpfiles, mut sysusers) = checked_in();
    mutation.apply(&mut tmpfiles, &mut sysusers);
    match validate_directory_layout(&tmpfiles, &sysusers) {
        Err(err) => err,
        Ok(()) => panic!("expected a validation failure"),
    }
}

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
    let err = validate_mutation(&mutation);
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
    let err = validate_mutation(&mutation);
    assert_eq!(err, expected);
    assert_snapshot!(err.to_string(), @"tmpfiles.d/repovec.conf must set mode explicitly on line 7");
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
    let err = validate_mutation(&mutation);
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
    let err = validate_mutation(&mutation);
    assert_eq!(err, expected);
    assert_snapshot!(err.to_string(), @"/var/lib/repovec/.grepai must be in group repovec: wheel");
}

#[test]
fn missing_directory_display_is_snapshotted() {
    let mutation = Mutation::tmpfiles("d /var/lib/repovec/worktrees 0700 repovec repovec -\n", "");
    let expected = DirectoryLayoutError::MissingDirectoryEntry {
        path: Utf8PathBuf::from("/var/lib/repovec/worktrees"),
    };
    let err = validate_mutation(&mutation);
    assert_eq!(err, expected);
    assert_snapshot!(err.to_string(), @"missing directory entry for /var/lib/repovec/worktrees");
}

#[test]
fn unexpected_directory_display_is_snapshotted() {
    let mutation = Mutation::tmpfiles("\n", "\nd /var/lib/repovec/cache 0700 repovec repovec -\n");
    let expected = DirectoryLayoutError::UnexpectedDirectoryEntry {
        path: Utf8PathBuf::from("/var/lib/repovec/cache"),
    };
    let err = validate_mutation(&mutation);
    assert_eq!(err, expected);
    assert_snapshot!(err.to_string(), @"unexpected directory entry for /var/lib/repovec/cache");
}

#[test]
fn forbidden_secrets_display_is_snapshotted() {
    let mutation = Mutation::tmpfiles("\n", "\nd /etc/repovec 0750 root repovec -\n");
    let expected =
        DirectoryLayoutError::ForbiddenSecretsEntry { path: Utf8PathBuf::from("/etc/repovec") };
    let err = validate_mutation(&mutation);
    assert_eq!(err, expected);
    assert_snapshot!(err.to_string(), @"/etc/repovec must not be declared by the tmpfiles.d asset");
}

#[test]
fn malformed_line_display_is_snapshotted() {
    let mutation = Mutation::tmpfiles("\n", "\nx /var/lib/repovec/bogus 0700 repovec repovec -\n");
    let expected = DirectoryLayoutError::MalformedLine {
        asset: "tmpfiles.d/repovec.conf",
        line_number: 2,
        line: "x /var/lib/repovec/bogus 0700 repovec repovec -".to_owned(),
    };
    let err = validate_mutation(&mutation);
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
    let err = validate_mutation(&mutation);
    assert_eq!(err, DirectoryLayoutError::SysusersMissingUser);
    assert_snapshot!(err.to_string(), @"the sysusers.d asset must declare the repovec user");
}

#[test]
fn sysusers_incorrect_home_display_is_snapshotted() {
    let mutation =
        Mutation::sysusers("/var/lib/repovec /usr/sbin/nologin", "/home/repovec /usr/sbin/nologin");
    let expected = DirectoryLayoutError::SysusersIncorrectHome {
        expected: "/var/lib/repovec",
        actual: "/home/repovec".to_owned(),
    };
    let err = validate_mutation(&mutation);
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
    let err = validate_mutation(&mutation);
    assert_eq!(err, expected);
    assert_snapshot!(err.to_string(), @"the repovec user shell must be /usr/sbin/nologin: /bin/bash");
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

fn parse_mode(text: &str) -> Result<Mode, DirectoryLayoutError> {
    let value = u16::from_str_radix(text, 8).map_err(|_| DirectoryLayoutError::MalformedLine {
        asset: "tmpfiles.d/repovec.conf",
        line_number: 0,
        line: text.to_owned(),
    })?;
    Ok(Mode(value))
}
