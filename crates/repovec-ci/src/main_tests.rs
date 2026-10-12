//! Unit coverage for CLI argument parsing.

use std::io;

use insta::assert_snapshot;
use repovec_ci::{MermaidDetection, evaluate_docs_gate_with};

use super::{
    Command, Input, USAGE, parse_args, print_usage, run_systemd_gate, run_systemd_gate_with,
    write_plan,
};

fn buffer_to_string(buffer: Vec<u8>) -> String {
    match String::from_utf8(buffer) {
        Ok(output) => output,
        Err(error) => panic!("snapshot buffer should contain UTF-8: {error}"),
    }
}

#[test]
fn test_help_output() {
    let mut buffer = Vec::new();

    print_usage(&mut buffer).expect("help output should be written");

    let output = buffer_to_string(buffer);

    assert_eq!(output, USAGE);
    assert_snapshot!("help_output", output);
}

#[test]
fn test_changed_file_docs_output() {
    let plan = evaluate_docs_gate_with(["docs/roadmap.md"], |_path| MermaidDetection::Absent);
    let mut buffer = Vec::new();

    write_plan(&mut buffer, &plan).expect("plan output should be written");

    assert_snapshot!("changed_file_docs_output", buffer_to_string(buffer));
}

#[test]
fn test_stdin_empty_output() {
    let plan =
        evaluate_docs_gate_with(std::iter::empty::<&str>(), |_path| MermaidDetection::Absent);
    let mut buffer = Vec::new();

    write_plan(&mut buffer, &plan).expect("plan output should be written");

    assert_snapshot!("stdin_empty_output", buffer_to_string(buffer));
}

#[test]
fn test_invalid_flag_error() {
    let Err(error) = parse_args(["--unknown"].into_iter().map(str::to_owned)) else {
        panic!("invalid flag should fail");
    };

    assert_snapshot!("invalid_flag_error", error.to_string());
}

#[test]
fn no_arguments_default_to_changed_files_input() {
    let command = parse_args(std::iter::empty::<String>()).expect("empty args should parse");

    match command {
        Command::DocsGate(Input::ChangedFiles(paths)) => assert!(paths.is_empty()),
        Command::Help | Command::DocsGate(_) | Command::SystemdGate => {
            panic!("empty args should default to ChangedFiles");
        }
    }
}

#[test]
fn help_flag_returns_help_input() {
    let command = parse_args(["--help".to_owned()]).expect("help flag should parse");

    assert!(matches!(command, Command::Help));
}

#[test]
fn docs_gate_subcommand_accepts_help_flag() {
    let command =
        parse_args(["docs-gate".to_owned(), "--help".to_owned()]).expect("help should parse");

    assert!(matches!(command, Command::DocsGate(Input::Help)));
}

#[test]
fn stdin_flag_returns_stdin_input() {
    let command = parse_args(["--stdin".to_owned()]).expect("stdin flag should parse");

    assert!(matches!(command, Command::DocsGate(Input::Stdin)));
}

#[test]
fn docs_gate_subcommand_accepts_stdin_flag() {
    let command =
        parse_args(["docs-gate".to_owned(), "--stdin".to_owned()]).expect("stdin should parse");

    assert!(matches!(command, Command::DocsGate(Input::Stdin)));
}

#[test]
fn systemd_gate_subcommand_returns_systemd_command() {
    let command = parse_args(["systemd-gate".to_owned()]).expect("systemd gate should parse");

    assert!(matches!(command, Command::SystemdGate));
}

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
            assert_eq!(paths, vec!["docs/users-guide.md", ".markdownlint-cli2.jsonc"]);
        }
        Command::Help | Command::DocsGate(_) | Command::SystemdGate => {
            panic!("changed-file flags should yield ChangedFiles");
        }
    }
}

#[test]
fn systemd_gate_rejects_changed_file_flags() {
    let Err(error) = parse_args([
        "systemd-gate".to_owned(),
        "--changed-file".to_owned(),
        "docs/users-guide.md".to_owned(),
    ]) else {
        panic!("systemd-gate should reject docs-gate flags");
    };

    assert_eq!(error.kind(), io::ErrorKind::InvalidInput);
    assert!(error.to_string().contains("unsupported argument for systemd-gate"));
}

#[test]
fn systemd_gate_validates_real_checked_in_units() {
    let mut buffer = Vec::new();

    run_systemd_gate(&mut buffer)
        .expect("checked-in systemd units must satisfy the appliance contract");

    assert_snapshot!("systemd_gate_real_validation_success", buffer_to_string(buffer));
}

#[test]
fn systemd_gate_writes_success_confirmation() {
    let mut buffer = Vec::new();

    run_systemd_gate_with(&mut buffer, || Ok(()))
        .expect("successful validation should write a confirmation line");

    assert_snapshot!("systemd_gate_success_confirmation", buffer_to_string(buffer));
}

#[test]
fn systemd_gate_propagates_validation_error() {
    use repovec_core::appliance::systemd_units::SystemdUnitError;

    let injected =
        SystemdUnitError::MissingSection { unit: "repovecd.service", section: "Service" };
    let mut buffer = Vec::new();

    let result = run_systemd_gate_with(&mut buffer, || Err(injected));

    assert!(result.is_err(), "validation failure must propagate as Err");
    assert!(buffer.is_empty(), "no output should be written on failure");
    let Err(error) = result else {
        panic!("validation failure must propagate as Err");
    };
    assert_snapshot!("systemd_gate_error_message", error.to_string());
}

#[test]
fn missing_changed_file_value_reports_usage() {
    let Err(error) = parse_args(["--changed-file".to_owned()]) else {
        panic!("missing changed-file value should fail");
    };

    assert_eq!(error.kind(), io::ErrorKind::InvalidInput);
    assert!(error.to_string().contains(USAGE));
}

#[test]
fn stdin_and_changed_file_are_mutually_exclusive() {
    let Err(error) = parse_args([
        "--stdin".to_owned(),
        "--changed-file".to_owned(),
        "docs/users-guide.md".to_owned(),
    ]) else {
        panic!("stdin and changed-file should not be combinable");
    };

    assert_eq!(error.kind(), io::ErrorKind::InvalidInput);
    assert!(error.to_string().contains("--stdin cannot be combined with --changed-file"));
}

#[test]
fn invalid_flag_reports_usage() {
    let Err(error) = parse_args(["--bogus".to_owned()]) else {
        panic!("invalid flag should fail");
    };

    assert_eq!(error.kind(), io::ErrorKind::InvalidInput);
    assert!(error.to_string().contains(USAGE));
}
