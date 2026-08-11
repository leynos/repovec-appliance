//! Contract tests for `validate_directory_layout`: deterministic mutations of
//! the checked-in assets against the per-directory appliance contract.

use camino::Utf8PathBuf;
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
            *tmpfiles = tmpfiles.replace(from, self.tmpfiles_to);
        }
        if let Some(from) = self.sysusers_from {
            *sysusers = sysusers.replace(from, self.sysusers_to);
        }
    }
}

fn checked_in() -> (String, String) {
    (checked_in_repovec_tmpfiles().to_owned(), checked_in_repovec_sysusers().to_owned())
}

fn assert_mutation(mutation: Mutation, expected: DirectoryLayoutError) {
    let (tmpfiles, sysusers) = checked_in();
    let mut tmpfiles = tmpfiles;
    let mut sysusers = sysusers;
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
                line_number: 4,
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
        assert_mutation(mutation, expected);
    }
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
