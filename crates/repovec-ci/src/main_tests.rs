//! Unit coverage for CLI argument parsing.

use std::{
    cell::{Cell, RefCell},
    io,
};

use insta::assert_snapshot;
use repovec_ci::{MermaidDetection, evaluate_docs_gate_with};
use rstest::rstest;

use super::{
    Command,
    Input,
    USAGE,
    parse_args,
    print_usage,
    run_systemd_gate,
    run_systemd_gate_with,
    write_plan,
};

/// Decode captured CLI output strictly so snapshots cannot conceal invalid UTF-8.
///
/// For example, the bytes in `b"ready"` produce `Ok("ready")`; invalid bytes are
/// reported rather than replaced with substitution characters, and each test
/// body turns the error into a panic at the call site.
fn buffer_to_string(buffer: Vec<u8>) -> Result<String, std::string::FromUtf8Error> {
    String::from_utf8(buffer)
}

/// Render the documentation plan for `paths` and pin it as `snapshot_name`.
fn assert_docs_plan_snapshot(snapshot_name: &str, paths: impl IntoIterator<Item = &'static str>) {
    let plan = evaluate_docs_gate_with(paths, |_path| MermaidDetection::Absent);
    let mut buffer = Vec::new();

    write_plan(&mut buffer, &plan).expect("plan output should be written");

    assert_snapshot!(
        snapshot_name,
        buffer_to_string(buffer).expect("plan output should be valid UTF-8")
    );
}

/// Assert a gate failure propagated without emitting success output.
fn assert_gate_failure(
    result: io::Result<()>,
    buffer: &[u8],
    failure_label: &str,
    expected_message: &str,
) {
    assert!(result.is_err(), "{failure_label} must propagate as Err");
    assert!(buffer.is_empty(), "no output should be written on failure");
    let Err(error) = result else {
        panic!("{failure_label} must propagate as Err");
    };
    assert_eq!(error.to_string(), expected_message);
}

/// Protect the exact operator-facing help text and its snapshot against accidental CLI changes.
#[test]
fn test_help_output() {
    let mut buffer = Vec::new();

    print_usage(&mut buffer).expect("help output should be written");

    let output = buffer_to_string(buffer).expect("help output should be valid UTF-8");

    assert_eq!(output, USAGE);
    assert_snapshot!("help_output", output);
}

/// Protect the rendered documentation plan for a changed Markdown file, beyond successful input
/// parsing.
#[test]
fn test_changed_file_docs_output() {
    assert_docs_plan_snapshot("changed_file_docs_output", ["docs/roadmap.md"]);
}

/// Keep an empty changed-path iterator conservative: missing inputs must not silently bypass
/// documentation or Mermaid checks.
#[test]
fn test_stdin_empty_output() {
    assert_docs_plan_snapshot("stdin_empty_output", std::iter::empty::<&str>());
}

/// Preserve the parser diagnostic snapshot so unsupported flags remain understandable to operators.
#[test]
fn test_invalid_flag_error() {
    let Err(error) = parse_args(["--unknown"].into_iter().map(str::to_owned)) else {
        panic!("invalid flag should fail");
    };

    assert_snapshot!("invalid_flag_error", error.to_string());
}

/// Keep bare invocation in changed-file mode to preserve the CI wrapper input contract.
#[test]
fn no_arguments_default_to_changed_files_input() {
    let command = parse_args(std::iter::empty::<String>()).expect("empty args should parse");

    match command {
        Command::DocsGate(Input::ChangedFiles(paths)) => assert_eq!(paths, [] as [String; 0]),
        Command::Help | Command::DocsGate(_) | Command::SystemdGate => {
            panic!("empty args should default to ChangedFiles");
        }
    }
}

/// Keep top-level help as a parser result so rendering remains at the CLI boundary.
#[test]
fn help_flag_returns_help_input() {
    let command = parse_args(["--help".to_owned()]).expect("help flag should parse");

    assert!(matches!(command, Command::Help));
}

/// Protect subcommand help from being mistaken for documentation-gate input.
#[test]
fn docs_gate_subcommand_accepts_help_flag() {
    let command =
        parse_args(["docs-gate".to_owned(), "--help".to_owned()]).expect("help should parse");

    assert!(matches!(command, Command::DocsGate(Input::Help)));
}

/// Preserve the shorthand that selects streamed documentation input for CI callers.
#[test]
fn stdin_flag_returns_stdin_input() {
    let command = parse_args(["--stdin".to_owned()]).expect("stdin flag should parse");

    assert!(matches!(command, Command::DocsGate(Input::Stdin)));
}

/// Keep explicit subcommand and shorthand syntax consistent about streamed input.
#[test]
fn docs_gate_subcommand_accepts_stdin_flag() {
    let command =
        parse_args(["docs-gate".to_owned(), "--stdin".to_owned()]).expect("stdin should parse");

    assert!(matches!(command, Command::DocsGate(Input::Stdin)));
}

/// Route systemd requests to their validator instead of accidentally producing a documentation
/// plan.
#[test]
fn systemd_gate_subcommand_returns_systemd_command() {
    let command = parse_args(["systemd-gate".to_owned()]).expect("systemd gate should parse");

    assert!(matches!(command, Command::SystemdGate));
}

/// Keep repeated path options ordered and complete, including documentation-tool configuration
/// inputs.
#[test]
fn changed_file_flags_collect_all_paths() {
    let command = parse_args([
        "--changed-file".to_owned(),
        "docs/users-guide.md".to_owned(),
        "--changed-file".to_owned(),
        ".markdownlint-cli2.jsonc".to_owned(),
    ])
    .expect("changed-file flags should parse");

    match command {
        Command::DocsGate(Input::ChangedFiles(paths)) => {
            assert_eq!(
                paths,
                vec!["docs/users-guide.md", ".markdownlint-cli2.jsonc"]
            );
        }
        Command::Help | Command::DocsGate(_) | Command::SystemdGate => {
            panic!("changed-file flags should yield ChangedFiles");
        }
    }
}

/// Reject documentation-only options in systemd mode and competing input sources in documentation
/// mode, rather than silently ignoring a caller mistake or combining ambiguous inputs.
#[rstest]
#[case::systemd_gate_flag(
    &["systemd-gate", "--changed-file", "docs/users-guide.md"],
    "unsupported argument for systemd-gate",
)]
#[case::competing_input_sources(
    &["--stdin", "--changed-file", "docs/users-guide.md"],
    "--stdin cannot be combined with --changed-file",
)]
fn conflicting_arguments_are_rejected(#[case] arguments: &[&str], #[case] expected_fragment: &str) {
    let Err(error) = parse_args(arguments.iter().copied().map(str::to_owned)) else {
        panic!("rejected arguments should fail with a parser error");
    };

    assert_eq!(error.kind(), io::ErrorKind::InvalidInput);
    assert!(error.to_string().contains(expected_fragment));
}

/// Keep the real systemd command wired to shipped unit validation rather than a test-only success
/// seam.
#[test]
fn systemd_gate_validates_real_checked_in_units() {
    let mut buffer = Vec::new();

    run_systemd_gate(&mut buffer)
        .expect("checked-in systemd units must satisfy the appliance contract");

    assert_snapshot!(
        "systemd_gate_real_validation_success",
        buffer_to_string(buffer).expect("gate output should be valid UTF-8")
    );
}

/// Keep successful validation observable to operators through a stable confirmation line.
#[test]
fn systemd_gate_writes_success_confirmation() {
    let mut buffer = Vec::new();

    run_systemd_gate_with(&mut buffer, || Ok(()), || Ok(()))
        .expect("successful validation should write a confirmation line");

    assert_snapshot!(
        "systemd_gate_success_confirmation",
        buffer_to_string(buffer).expect("gate output should be valid UTF-8")
    );
}

/// Keep unit validation first and directory validation second in the CI gate.
#[test]
fn systemd_gate_checks_units_before_directory_layout() {
    let observed = RefCell::new(Vec::new());
    let mut buffer = Vec::new();

    run_systemd_gate_with(
        &mut buffer,
        || {
            observed.borrow_mut().push("systemd units");
            Ok(())
        },
        || {
            observed.borrow_mut().push("directory layout");
            Ok(())
        },
    )
    .expect("both validators should succeed");

    assert_eq!(*observed.borrow(), ["systemd units", "directory layout"]);
}

/// A unit-contract failure must stop before directory validation and success output.
#[test]
fn systemd_gate_propagates_unit_validation_error() {
    let layout_called = Cell::new(false);
    let mut buffer = Vec::new();

    let result = run_systemd_gate_with(
        &mut buffer,
        || Err(io::Error::other("repovecd.service is missing [Service]")),
        || {
            layout_called.set(true);
            Ok(())
        },
    );

    assert!(
        !layout_called.get(),
        "layout validation must follow unit validation"
    );
    assert_gate_failure(
        result,
        &buffer,
        "unit validation failure",
        "repovecd.service is missing [Service]",
    );
}

/// A layout-contract failure must propagate without emitting success output.
#[test]
fn systemd_gate_propagates_directory_layout_validation_error() {
    let units_called = Cell::new(false);
    let mut buffer = Vec::new();

    let result = run_systemd_gate_with(
        &mut buffer,
        || {
            units_called.set(true);
            Ok(())
        },
        || Err(io::Error::other("/var/lib/repovec has mode 0755")),
    );

    assert!(
        units_called.get(),
        "unit validation must run before layout validation"
    );
    assert_gate_failure(
        result,
        &buffer,
        "layout validation failure",
        "/var/lib/repovec has mode 0755",
    );
}

/// Reject dangling path options with usage guidance instead of interpreting them as empty paths.
#[test]
fn missing_changed_file_value_reports_usage() {
    let Err(error) = parse_args(["--changed-file".to_owned()]) else {
        panic!("missing changed-file value should fail");
    };

    assert_eq!(error.kind(), io::ErrorKind::InvalidInput);
    assert!(error.to_string().contains(USAGE));
}

/// Keep usage guidance attached to unknown-option errors so callers can recover from invalid CLI
/// syntax.
#[test]
fn invalid_flag_reports_usage() {
    let Err(error) = parse_args(["--bogus".to_owned()]) else {
        panic!("invalid flag should fail");
    };

    assert_eq!(error.kind(), io::ErrorKind::InvalidInput);
    assert!(error.to_string().contains(USAGE));
}
