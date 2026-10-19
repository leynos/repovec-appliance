//! Unit tests for live account resolution and operator-facing errors.
//!
//! The resolution rules are driven from captured database text, so no test
//! depends on the host's real `/etc/passwd` or `/etc/group`. The filesystem
//! half of the pre-flight lives in [`super::tests_live_tree`].

use camino::Utf8PathBuf;

use super::{
    group::find_group_gid,
    live::{LiveLayoutError, LiveLayoutIds, LiveLayoutViolation, resolve_ids_from_databases},
};

/// The group database, not the user's primary gid, supplies the expected group.
///
/// `systemd-tmpfiles` resolves its group column by name, so a correctly
/// provisioned host may carry a `repovec` user whose primary group differs from
/// the `repovec` group. Deriving the expectation from the passwd gid would
/// reject that host.
#[test]
fn resolution_from_databases_uses_the_group_database_not_the_user_gid() {
    let passwd = "repovec:x:998:1:repovec:/var/lib/repovec:/usr/sbin/nologin\n";
    let group = "repovec:x:996:\n";

    let ids = resolve_ids_from_databases(passwd, group).expect("resolvable ids");

    assert_eq!(
        ids,
        LiveLayoutIds { repovec_uid: 998, repovec_gid: 996, privileged_uid: 0, privileged_gid: 0 }
    );
}

/// An absent account row stops resolution with the typed error.
#[test]
fn resolution_reports_a_missing_account() {
    let result =
        resolve_ids_from_databases("root:x:0:0:root:/root:/bin/bash\n", "repovec:x:996:\n");

    assert!(matches!(result, Err(LiveLayoutError::RepovecUserNotFound)));
}

/// A truncated account row is not proof of provisioning, so it is rejected.
#[test]
fn resolution_rejects_a_truncated_account() {
    let result = resolve_ids_from_databases("repovec:x:998:996:repovec\n", "repovec:x:996:\n");

    assert!(matches!(result, Err(LiveLayoutError::RepovecUserNotFound)));
}

/// An absent group row stops resolution with the typed error.
#[test]
fn resolution_reports_a_missing_group() {
    let passwd = "repovec:x:998:996:repovec:/var/lib/repovec:/usr/sbin/nologin\n";

    let result = resolve_ids_from_databases(passwd, "root:x:0:\n");

    assert!(matches!(result, Err(LiveLayoutError::RepovecGroupNotFound)));
}

/// A root-owned account is rejected before the group database is consulted.
#[test]
fn resolution_rejects_an_unsafe_account_before_resolving_the_group() {
    let passwd = "repovec:x:0:0:repovec:/var/lib/repovec:/usr/sbin/nologin\n";

    let result = resolve_ids_from_databases(passwd, "repovec:x:996:\n");

    assert!(matches!(result, Err(LiveLayoutError::RepovecUserIsRoot { uid: 0 })));
}

/// A pre-existing account with the wrong home is rejected, because
/// `systemd-sysusers` leaves existing accounts untouched.
#[test]
fn resolution_rejects_an_incorrect_home() {
    let passwd = "repovec:x:998:996:repovec:/home/repovec:/usr/sbin/nologin\n";

    let result = resolve_ids_from_databases(passwd, "repovec:x:996:\n");

    let Err(LiveLayoutError::RepovecHomeIncorrect { expected, actual }) = result else {
        panic!("expected a home violation, got {result:?}");
    };
    assert_eq!(expected, "/var/lib/repovec");
    assert_eq!(actual, "/home/repovec");
}

/// A pre-existing account with a login shell is rejected for the same reason.
#[test]
fn resolution_rejects_a_login_shell() {
    let passwd = "repovec:x:998:996:repovec:/var/lib/repovec:/bin/bash\n";

    let result = resolve_ids_from_databases(passwd, "repovec:x:996:\n");

    assert!(matches!(
        result,
        Err(LiveLayoutError::RepovecShellIncorrect { expected: "/usr/sbin/nologin", .. })
    ));
}

/// Every variant carries a stable, bounded category for metrics and logs.
#[test]
fn every_error_variant_has_a_stable_category() {
    let violation = || LiveLayoutViolation {
        path: Utf8PathBuf::from("/var/lib/repovec"),
        expected: "repovec (uid 998)".to_owned(),
        actual: "uid 0".to_owned(),
    };
    let cases = [
        (LiveLayoutError::RepovecUserNotFound, "repovec_user_not_found"),
        (LiveLayoutError::RepovecGroupNotFound, "repovec_group_not_found"),
        (LiveLayoutError::RepovecUserIsRoot { uid: 0 }, "repovec_user_is_root"),
        (
            LiveLayoutError::RepovecHomeIncorrect { expected: "/a", actual: "/b".to_owned() },
            "repovec_home_incorrect",
        ),
        (
            LiveLayoutError::RepovecShellIncorrect { expected: "/a", actual: "/b".to_owned() },
            "repovec_shell_incorrect",
        ),
        (LiveLayoutError::MissingPath { path: "/var/lib/repovec".into() }, "missing_path"),
        (LiveLayoutError::IncorrectMode { violation: violation() }, "incorrect_mode"),
        (LiveLayoutError::IncorrectOwner { violation: violation() }, "incorrect_owner"),
        (LiveLayoutError::IncorrectGroup { violation: violation() }, "incorrect_group"),
        (
            LiveLayoutError::UnreadablePath {
                path: "/var/lib/repovec".into(),
                source: std::io::Error::from(std::io::ErrorKind::PermissionDenied),
            },
            "unreadable_path",
        ),
        (LiveLayoutError::NotADirectory { path: "/var/lib/repovec".into() }, "not_a_directory"),
    ];

    for (error, expected) in cases {
        assert_eq!(error.category(), expected);
    }
}

/// The bounded diagnostic pair retains both sides of a comparison.
#[test]
fn diagnostic_values_preserve_expected_and_actual() {
    let violation = LiveLayoutViolation {
        path: Utf8PathBuf::from("/var/lib/repovec"),
        expected: "repovec (uid 998)".to_owned(),
        actual: "uid 0".to_owned(),
    };

    let error = LiveLayoutError::IncorrectOwner { violation };

    assert_eq!(error.diagnostic_values(), ("repovec (uid 998)", "uid 0"));
}

/// Errors without an observed value report empty diagnostics rather than
/// inventing one.
#[test]
fn diagnostic_values_are_empty_when_nothing_was_observed() {
    assert_eq!(LiveLayoutError::MissingPath { path: "/a".into() }.diagnostic_values(), ("", ""));
    assert_eq!(LiveLayoutError::RepovecUserNotFound.diagnostic_values(), ("", ""));
}

/// Pin the operator-facing rendering and path accessor for an ownership
/// violation.
#[test]
fn error_display_and_path_are_operator_facing() {
    let violation = LiveLayoutViolation {
        path: Utf8PathBuf::from("/var/lib/repovec"),
        expected: "repovec (uid 1000)".to_owned(),
        actual: "uid 0".to_owned(),
    };
    let error = LiveLayoutError::IncorrectOwner { violation };

    assert_eq!(error.to_string(), "/var/lib/repovec must be owned by repovec (uid 1000): uid 0");
    assert_eq!(error.path(), Some("/var/lib/repovec"));
}

/// Database-read failures name the database, not the directory under test.
#[test]
fn unreadable_database_display_names_the_database() {
    let error = LiveLayoutError::UnreadablePath {
        path: Utf8PathBuf::from("/etc/passwd"),
        source: std::io::Error::from(std::io::ErrorKind::PermissionDenied),
    };

    assert_eq!(error.to_string(), "cannot inspect /etc/passwd: permission denied");
    assert_eq!(error.category(), "unreadable_path");
}

/// The group parser reads the gid column of the named row.
#[test]
fn find_group_gid_parses_group_entry() {
    let sample = "root:x:0:\nwheel:x:10:leynos\nrepovec:x:996:\n";

    assert_eq!(find_group_gid(sample, "repovec"), Some(996));
}

/// The lookup does not depend on row order.
#[test]
fn find_group_gid_is_independent_of_row_order() {
    let sample = "repovec:x:996:\nroot:x:0:\n";

    assert_eq!(find_group_gid(sample, "repovec"), Some(996));
}

/// An absent group row yields no gid.
#[test]
fn find_group_gid_returns_none_when_absent() {
    let sample = "root:x:0:\nwheel:x:10:leynos\n";

    assert_eq!(find_group_gid(sample, "repovec"), None);
}

/// A prefix match must not be mistaken for the whole name.
#[test]
fn find_group_gid_ignores_a_prefix_match_on_the_name() {
    let sample = "repovec-backup:x:997:\nrepovec:x:996:\n";

    assert_eq!(find_group_gid(sample, "repovec"), Some(996));
}

/// A truncated group row is not trusted.
#[test]
fn find_group_gid_ignores_a_truncated_row() {
    let sample = "repovec:x\n";

    assert_eq!(find_group_gid(sample, "repovec"), None);
}

/// The group-lookup error keeps its operator-facing message.
#[test]
fn group_lookup_error_display_names_the_database() {
    let error = super::group::GroupLookupError::NotFound;

    assert_eq!(error.to_string(), "the repovec system group does not exist");
}
