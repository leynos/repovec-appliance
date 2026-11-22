//! Tests for the argument vector and the usage text.
//!
//! Every case is decided before a credential is acquired, so none of them
//! reaches GitHub or the encrypted user store.

use rstest::rstest;

use super::fixtures::{Arguments, EMPTY_INPUT, Harness, Rejection, run_without_input};

#[rstest]
#[case::no_mode(Rejection::NoMode)]
#[case::two_modes(Rejection::TwoModes)]
#[case::unknown_argument(Rejection::UnknownArgument)]
#[case::missing_store_directory(Rejection::MissingStoreDirectory)]
fn a_command_line_the_listing_cannot_act_on_is_a_usage_error(#[case] rejection: Rejection) {
    let outcome = run_without_input(rejection.arguments());

    assert_eq!(
        outcome.status, 2,
        "a rejected command line must exit 2 rather than run: {}",
        outcome.stderr
    );
    assert!(
        outcome.stdout.is_empty(),
        "a usage error must write nothing to standard output"
    );
    assert!(
        outcome.stderr.contains(rejection.fragment()),
        "the diagnostic must mention {:?}: {}",
        rejection.fragment(),
        outcome.stderr
    );
    assert_eq!(
        outcome.loader_calls, 0,
        "a usage error must not consult the credential store"
    );
}

#[rstest]
#[case::long(Arguments::long_help())]
#[case::short(Arguments::short_help())]
fn the_usage_text_documents_the_modes_and_the_budgets(#[case] arguments: Arguments) {
    let outcome = run_without_input(arguments);

    assert_eq!(
        outcome.status, 0,
        "the usage text must be available without a credential"
    );
    assert!(
        outcome.stderr.is_empty(),
        "the usage text must not report a diagnostic: {}",
        outcome.stderr
    );
    for expected in [
        "--user-store",
        "--installation-token-stdin",
        "10000 pages",
        "not a snapshot",
        "Exit status:",
    ] {
        assert!(
            outcome.stdout.contains(expected),
            "the usage text must mention {expected:?}: {}",
            outcome.stdout
        );
    }
}

#[cfg(unix)]
#[test]
fn a_non_utf8_argument_is_a_usage_error() {
    use std::{ffi::OsString, os::unix::ffi::OsStringExt};

    let arguments = Arguments::from_raw(vec![OsString::from_vec(b"--user-store=\xff".to_vec())]);

    let outcome = Harness::new(arguments, EMPTY_INPUT).run();

    assert_eq!(
        outcome.status, 2,
        "an argument that is not text cannot be acted on"
    );
    assert!(
        outcome.stderr.contains("must be valid UTF-8"),
        "the diagnostic must name the encoding rather than the argument: {}",
        outcome.stderr
    );
    assert_eq!(
        outcome.loader_calls, 0,
        "a usage error must not consult the credential store"
    );
}
