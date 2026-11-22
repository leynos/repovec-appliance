//! Tests for standard input, the credential store, and failing streams.
//!
//! Standard input is bounded and validated before any request, and the
//! credential store is consulted only for the mode that names it. The failing
//! streams prove that a lost diagnostic is reported as its own status rather
//! than as the failure it was meant to describe.

use rstest::rstest;

use super::fixtures::{
    Arguments,
    Harness,
    RefusedStream,
    UnusableInput,
    run_command_line,
    run_without_input,
};
use crate::repository_listing_command::MAX_INSTALLATION_TOKEN_BYTES;

#[rstest]
#[case::empty(UnusableInput::Empty)]
#[case::invalid_utf8(UnusableInput::InvalidUtf8)]
#[case::interior_whitespace(UnusableInput::InteriorWhitespace)]
fn unusable_standard_input_is_a_credential_failure(#[case] input: UnusableInput) {
    let outcome = run_command_line(Arguments::installation_stdin(), input.bytes());

    assert_eq!(
        outcome.status, 3,
        "unusable standard input must fail credential acquisition: {}",
        outcome.stderr
    );
    assert!(
        outcome.stdout.is_empty(),
        "a credential failure must write no repository output"
    );
    assert!(
        outcome.stderr.contains(input.fragment()),
        "the diagnostic must describe the rejected input with {:?}: {}",
        input.fragment(),
        outcome.stderr
    );
    assert_eq!(
        outcome.loader_calls, 0,
        "installation mode must never read the user store"
    );
}

#[test]
fn standard_input_over_the_bound_is_a_credential_failure() {
    let input = vec![b'a'; MAX_INSTALLATION_TOKEN_BYTES.saturating_add(1)];

    let outcome = run_command_line(Arguments::installation_stdin(), &input);

    assert_eq!(
        outcome.status, 3,
        "an oversize credential must fail credential acquisition: {}",
        outcome.stderr
    );
    assert!(
        outcome.stderr.contains("exceeds 16 KiB"),
        "the diagnostic must name the crossed limit: {}",
        outcome.stderr
    );
}

#[test]
fn an_unavailable_user_store_is_a_credential_failure() {
    let outcome = run_without_input(Arguments::user_store());

    assert_eq!(
        outcome.status, 3,
        "an unavailable user store must fail credential acquisition: {}",
        outcome.stderr
    );
    assert!(
        outcome.stdout.is_empty(),
        "a credential failure must write no repository output"
    );
    assert!(
        outcome.stderr.contains("could not be loaded"),
        "the diagnostic must name the credential source: {}",
        outcome.stderr
    );
    assert_eq!(
        outcome.loader_calls, 1,
        "user mode must consult the store exactly once"
    );
}

#[rstest]
#[case::usage_text(RefusedStream::Stdout, Arguments::long_help(), 5)]
#[case::usage_diagnostic(RefusedStream::Stderr, Arguments::none(), 6)]
fn a_stream_that_refuses_every_write_maps_to_its_own_failure(
    #[case] refused: RefusedStream,
    #[case] arguments: Arguments,
    #[case] status: u8,
) {
    let outcome = Harness::refusing(refused, arguments).run();

    assert_eq!(
        outcome.status, status,
        "{refused:?} refusing every write must be reported as its own failure"
    );
    assert!(
        outcome.stdout.is_empty(),
        "neither failure may claim to have written standard output"
    );
    assert!(
        outcome.stderr.is_empty(),
        "neither failure may claim to have written a diagnostic"
    );
}
