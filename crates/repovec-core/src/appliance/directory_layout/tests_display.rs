//! Operator-facing `Display` text for directory-layout validation errors.
//!
//! Every variant is rendered from a real `validate_directory_layout` failure, so
//! the human-readable contract text cannot drift unnoticed. The cases are held
//! as one table because they differ only in the mutation that produces the
//! error, the error the validator must return, and the sentence an operator
//! reads; the assertion path is identical for all of them.

use camino::Utf8PathBuf;
use rstest::rstest;

use super::tests_support::{Mutation, validate_mutation};
use crate::appliance::directory_layout::{DirectoryLayoutError, Mode};

/// One `Display`-contract case: a mutation, the error it must produce, and the
/// exact sentence an operator must be shown.
struct DisplayCase {
    /// The mutation that drives the validator into the failing state.
    mutation: Mutation,
    /// The error the validator must return for that mutation.
    expected: DirectoryLayoutError,
    /// The `Display` rendering of `expected`.
    display: &'static str,
}

/// Builds a case whose expected error renders as `display`.
const fn case(
    mutation: Mutation,
    expected: DirectoryLayoutError,
    display: &'static str,
) -> DisplayCase {
    DisplayCase { mutation, expected, display }
}

#[rstest]
#[case::incorrect_mode(case(
    Mutation::tmpfiles(
        "d /var/lib/repovec/worktrees 0700 repovec repovec -",
        "d /var/lib/repovec/worktrees 0750 repovec repovec -",
    ),
    DirectoryLayoutError::IncorrectMode {
        path: Utf8PathBuf::from("/var/lib/repovec/worktrees"),
        expected: Mode(0o700),
        actual: Mode(0o750),
    },
    "/var/lib/repovec/worktrees must have mode 0700: 0750",
))]
#[case::non_explicit_field(case(
    Mutation::tmpfiles(
        "d /var/lib/repovec/worktrees 0700 repovec repovec -",
        "d /var/lib/repovec/worktrees - repovec repovec -",
    ),
    DirectoryLayoutError::NonExplicitField {
        asset: "tmpfiles.d/repovec.conf",
        line_number: 7,
        field: "mode",
    },
    "tmpfiles.d/repovec.conf must set mode explicitly on line 7",
))]
#[case::invalid_field(case(
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
    "tmpfiles.d/repovec.conf has an invalid mode on line 7: 0999",
))]
#[case::incorrect_owner(case(
    Mutation::tmpfiles(
        "d /var/lib/repovec/git-mirrors 0700 repovec repovec -",
        "d /var/lib/repovec/git-mirrors 0700 root repovec -",
    ),
    DirectoryLayoutError::IncorrectOwner {
        path: Utf8PathBuf::from("/var/lib/repovec/git-mirrors"),
        expected: "repovec",
        actual: "root".to_owned(),
    },
    "/var/lib/repovec/git-mirrors must be owned by repovec: root",
))]
#[case::incorrect_group(case(
    Mutation::tmpfiles(
        "d /var/lib/repovec/.grepai 0700 repovec repovec -",
        "d /var/lib/repovec/.grepai 0700 repovec wheel -",
    ),
    DirectoryLayoutError::IncorrectGroup {
        path: Utf8PathBuf::from("/var/lib/repovec/.grepai"),
        expected: "repovec",
        actual: "wheel".to_owned(),
    },
    "/var/lib/repovec/.grepai must be in group repovec: wheel",
))]
#[case::missing_directory(case(
    Mutation::tmpfiles("d /var/lib/repovec/worktrees 0700 repovec repovec -\n", ""),
    DirectoryLayoutError::MissingDirectoryEntry {
        path: Utf8PathBuf::from("/var/lib/repovec/worktrees"),
    },
    "missing directory entry for /var/lib/repovec/worktrees",
))]
#[case::unexpected_directory(case(
    Mutation::tmpfiles("\n", "\nd /var/lib/repovec/cache 0700 repovec repovec -\n"),
    DirectoryLayoutError::UnexpectedDirectoryEntry {
        path: Utf8PathBuf::from("/var/lib/repovec/cache"),
    },
    "unexpected directory entry for /var/lib/repovec/cache",
))]
#[case::forbidden_secrets_entry(case(
    Mutation::tmpfiles("\n", "\nd /etc/repovec 0750 root repovec -\n"),
    DirectoryLayoutError::ForbiddenSecretsEntry { path: Utf8PathBuf::from("/etc/repovec") },
    "/etc/repovec must not be declared by the tmpfiles.d/repovec.conf asset",
))]
#[case::malformed_line(case(
    Mutation::tmpfiles("\n", "\nx /var/lib/repovec/bogus 0700 repovec repovec -\n"),
    DirectoryLayoutError::MalformedLine {
        asset: "tmpfiles.d/repovec.conf",
        line_number: 2,
        line: "x /var/lib/repovec/bogus 0700 repovec repovec -".to_owned(),
    },
    "malformed line in tmpfiles.d/repovec.conf at 2: x /var/lib/repovec/bogus 0700 repovec repovec -",
))]
#[case::sysusers_missing_user(case(
    Mutation::sysusers(
        "u repovec - \"repovec appliance service user\" /var/lib/repovec /usr/sbin/nologin",
        "u other - \"repovec appliance service user\" /var/lib/repovec /usr/sbin/nologin",
    ),
    DirectoryLayoutError::SysusersMissingUser,
    "the sysusers.d/repovec.conf asset must declare the repovec user",
))]
#[case::sysusers_pinned_uid(case(
    Mutation::sysusers("u repovec - ", "u repovec 1000 "),
    DirectoryLayoutError::SysusersPinnedUid { actual: "1000".to_owned() },
    "the repovec user uid must be allocated by systemd-sysusers, not pinned: 1000",
))]
#[case::sysusers_incorrect_home(case(
    Mutation::sysusers("/var/lib/repovec /usr/sbin/nologin", "/home/repovec /usr/sbin/nologin"),
    DirectoryLayoutError::SysusersIncorrectHome {
        expected: "/var/lib/repovec",
        actual: "/home/repovec".to_owned(),
    },
    "the repovec user home must be /var/lib/repovec: /home/repovec",
))]
#[case::sysusers_incorrect_shell(case(
    Mutation::sysusers("/usr/sbin/nologin", "/bin/bash"),
    DirectoryLayoutError::SysusersIncorrectShell {
        expected: "/usr/sbin/nologin",
        actual: "/bin/bash".to_owned(),
    },
    "the repovec user shell must be /usr/sbin/nologin: /bin/bash",
))]
fn display_messages_are_operator_facing(#[case] case: DisplayCase) {
    let error = validate_mutation(case.mutation);

    assert_eq!(error, case.expected);
    assert_eq!(error.to_string(), case.display);
}
