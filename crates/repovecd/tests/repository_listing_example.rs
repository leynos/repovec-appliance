//! Subprocess tests for the runnable listing example.
//!
//! These exercise the parts only the real child process can show: argument
//! handling, the usage text, and the bounded standard-input reader on standard
//! input. Every case here is rejected before a request is attempted, so no
//! test depends on network access. Successful loopback runs go through the
//! shared composition in `repository_listing_e2e` instead.

use std::{
    io::{self, Write},
    path::{Path, PathBuf},
    process::{Command, Output, Stdio},
    time::{Duration, Instant},
};

use repovecd::repository_listing_command::MAX_INSTALLATION_TOKEN_BYTES;

/// How long a child process may run before it is killed and the test fails.
const CHILD_DEADLINE: Duration = Duration::from_secs(30);

/// How long the parent waits between checks for the child's exit.
const CHILD_POLL_INTERVAL: Duration = Duration::from_millis(10);

/// Returns the example binary built beside this test binary's profile.
///
/// Cargo places the test binary and the example in the same profile directory,
/// but not necessarily at the same depth: with metadata embedding disabled the
/// test binary sits under `build/<package>/<hash>/out`. Walking the ancestors
/// keeps the rule independent of that layout and of a custom target directory.
///
/// # Errors
///
/// Returns an error naming the build command when no example binary is found.
fn example_binary() -> Result<PathBuf, io::Error> {
    let test_binary = std::env::current_exe()?;
    let relative = Path::new("examples").join(format!(
        "list_github_repositories{}",
        std::env::consts::EXE_SUFFIX
    ));
    let found = test_binary
        .ancestors()
        .skip(1)
        .map(|ancestor| ancestor.join(&relative))
        .find(|candidate| candidate.is_file());
    found.ok_or_else(|| {
        io::Error::other(format!(
            "no example binary found beside {}; build it with `cargo build -p repovecd --example \
             list_github_repositories`",
            test_binary.display()
        ))
    })
}

/// Runs the example to completion with cleared environment and piped streams.
///
/// # Errors
///
/// Returns an I/O error when the child cannot be started, written to, or
/// collected, or when it outlives [`CHILD_DEADLINE`].
fn run_example(arguments: &[&str], stdin: &[u8]) -> Result<Output, io::Error> {
    let mut child = Command::new(example_binary()?)
        .args(arguments)
        .env_clear()
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;
    let mut input = child
        .stdin
        .take()
        .ok_or_else(|| io::Error::other("standard input is piped to the child"))?;
    input.write_all(stdin)?;
    drop(input);
    let started = Instant::now();
    loop {
        if child.try_wait()?.is_some() {
            break;
        }
        if started.elapsed() > CHILD_DEADLINE {
            drop(child.kill());
            drop(child.wait());
            return Err(io::Error::other(format!(
                "the example did not exit within {CHILD_DEADLINE:?}"
            )));
        }
        std::thread::sleep(CHILD_POLL_INTERVAL);
    }
    child.wait_with_output()
}

/// Returns the child's standard output as text.
fn stdout_text(output: &Output) -> String { String::from_utf8_lossy(&output.stdout).into_owned() }

/// Returns the child's standard error as text.
fn stderr_text(output: &Output) -> String { String::from_utf8_lossy(&output.stderr).into_owned() }

#[test]
fn no_arguments_is_a_usage_error() {
    let output = run_example(&[], b"").expect("the example should run");

    assert_eq!(
        output.status.code(),
        Some(2),
        "a missing credential mode must be reported as a usage error"
    );
    assert!(
        stdout_text(&output).is_empty(),
        "a usage error must write nothing to standard output"
    );
    assert!(
        stderr_text(&output).contains("exactly one credential mode is required"),
        "the diagnostic must name the missing mode: {}",
        stderr_text(&output)
    );
}

#[test]
fn two_credential_modes_are_a_usage_error() {
    let output = run_example(
        &["--installation-token-stdin", "--user-store", "/etc/repovec"],
        b"",
    )
    .expect("the example should run");

    assert_eq!(
        output.status.code(),
        Some(2),
        "a second credential mode must be rejected"
    );
    assert!(
        stderr_text(&output).contains("exactly one credential mode is required"),
        "the diagnostic must name the conflicting modes: {}",
        stderr_text(&output)
    );
}

#[test]
fn an_unknown_argument_is_a_usage_error() {
    let output = run_example(&["--token", "literal"], b"").expect("the example should run");

    assert_eq!(
        output.status.code(),
        Some(2),
        "a literal token argument is not offered and must be rejected"
    );
    assert!(
        stderr_text(&output).contains("unknown argument"),
        "the diagnostic must name the rejected argument: {}",
        stderr_text(&output)
    );
}

#[test]
fn help_succeeds_and_documents_both_modes_and_the_budgets() {
    let output = run_example(&["--help"], b"").expect("the example should run");

    assert_eq!(
        output.status.code(),
        Some(0),
        "the usage text must be available without a credential"
    );
    let help = stdout_text(&output);
    for expected in [
        "--user-store",
        "--installation-token-stdin",
        "10000 pages",
        "not a snapshot",
    ] {
        assert!(
            help.contains(expected),
            "the usage text must mention {expected}: {help}"
        );
    }
}

#[test]
fn empty_standard_input_fails_before_networking() {
    let output = run_example(&["--installation-token-stdin"], b"").expect("the example should run");

    assert_eq!(
        output.status.code(),
        Some(3),
        "an empty credential must fail credential acquisition"
    );
    assert!(
        stdout_text(&output).is_empty(),
        "a credential failure must write no repository output"
    );
    assert!(
        stderr_text(&output).contains("standard input"),
        "the diagnostic must name the credential source: {}",
        stderr_text(&output)
    );
}

#[test]
fn oversize_standard_input_fails_before_networking() {
    let input = vec![b'a'; MAX_INSTALLATION_TOKEN_BYTES.saturating_add(1)];
    let output =
        run_example(&["--installation-token-stdin"], &input).expect("the example should run");

    assert_eq!(
        output.status.code(),
        Some(3),
        "an oversize credential must fail credential acquisition"
    );
    assert!(
        stderr_text(&output).contains("exceeds 16 KiB"),
        "the diagnostic must name the crossed limit: {}",
        stderr_text(&output)
    );
}

#[test]
fn invalid_utf8_standard_input_fails_before_networking() {
    let output = run_example(&["--installation-token-stdin"], &[0xff, 0xfe])
        .expect("the example should run");

    assert_eq!(
        output.status.code(),
        Some(3),
        "a non-UTF-8 credential must fail credential acquisition"
    );
    assert!(
        stderr_text(&output).contains("not valid UTF-8"),
        "the diagnostic must name the encoding: {}",
        stderr_text(&output)
    );
}
