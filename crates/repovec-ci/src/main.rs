//! Command-line entry point for CI policy helpers.
//!
//! This binary dispatches small repository policy gates used by local
//! Makefile targets and GitHub Actions. With no subcommand, or with
//! `docs-gate`, it evaluates changed paths through the `repovec-ci` library and
//! prints GitHub Actions-compatible key/value output. With `systemd-gate`, it
//! calls [`repovec_core::appliance::systemd_units`] to validate the checked-in
//! appliance unit files outside the test runner.
//!
//! The dispatcher keeps CI wiring in one binary while preserving narrow
//! command handlers: argument parsing selects a [`Command`], docs-gate planning
//! stays in the `repovec-ci` library, and systemd contract validation remains
//! owned by `repovec-core`.

use std::io::{self, BufRead as _, BufWriter, Write as _};

use cap_std::{ambient_authority, fs_utf8::Dir};
use repovec_ci::{DocsGatePlan, evaluate_docs_gate_in};

/// Help text describing the accepted commands and changed-file input modes.
const USAGE: &str = concat!(
    "Usage: repovec-ci [docs-gate] [--changed-file <path> [--changed-file <path>]...] [--help]\n",
    "       repovec-ci [docs-gate] --stdin\n",
    "       repovec-ci systemd-gate\n\n",
    "Reads a changed-file list and prints documentation-gate decisions in\n",
    "GitHub Actions output format, or validates checked-in systemd units.\n"
);

fn main() {
    if let Err(error) = run() {
        match writeln!(io::stderr().lock(), "{error}") {
            Ok(()) | Err(_) => {}
        }
        std::process::exit(1);
    }
}

/// Dispatches the process arguments and writes the selected gate output.
///
/// # Example
/// With `systemd-gate`, validates the checked-in units and prints the success message.
fn run() -> io::Result<()> {
    let mut stdout = BufWriter::new(io::stdout().lock());
    let command = parse_args(std::env::args().skip(1))?;
    match command {
        Command::Help => print_usage(&mut stdout),
        Command::DocsGate(input) => run_docs_gate(&mut stdout, input),
        Command::SystemdGate => run_systemd_gate(&mut stdout),
    }
}

/// Repository policy action selected from the command-line arguments.
enum Command {
    /// Displays command usage without evaluating a policy gate.
    Help,
    /// Evaluates documentation policy using the selected changed-file input.
    DocsGate(Input),
    /// Validates the checked-in appliance systemd units.
    SystemdGate,
}

/// Source of changed paths for the documentation gate.
enum Input {
    /// Displays command usage without evaluating a policy gate.
    Help,
    /// Paths supplied explicitly through repeated `--changed-file` arguments.
    ChangedFiles(Vec<String>),
    /// One changed path per standard-input line.
    Stdin,
}

/// Writes and flushes the command help text to the supplied output.
///
/// # Example
/// With a byte buffer as output, produces the bytes of `USAGE`.
fn print_usage(out: &mut impl io::Write) -> io::Result<()> {
    out.write_all(USAGE.as_bytes())?;
    out.flush()
}

/// Evaluates changed paths against the current repository directory.
///
/// # Example
/// A changed `README.md` requests the documentation gate in the returned plan.
fn evaluate_from_paths(paths: Vec<String>) -> io::Result<DocsGatePlan> {
    let root = Dir::open_ambient_dir(".", ambient_authority())?;
    Ok(evaluate_docs_gate_in(&root, paths))
}

/// Evaluates standard-input paths against the current repository directory.
///
/// # Example
/// An input line containing `README.md` requests the documentation gate.
fn evaluate_from_stdin() -> io::Result<DocsGatePlan> {
    let root = Dir::open_ambient_dir(".", ambient_authority())?;
    Ok(evaluate_docs_gate_in(&root, read_paths_from_stdin()?))
}

/// Writes either help text or a documentation plan for the selected input.
///
/// # Example
/// `Input::Stdin` reads changed paths and writes the resulting gate decisions.
fn run_docs_gate(out: &mut impl io::Write, input: Input) -> io::Result<()> {
    match input {
        Input::Help => print_usage(out),
        Input::ChangedFiles(paths) => write_plan(out, &evaluate_from_paths(paths)?),
        Input::Stdin => write_plan(out, &evaluate_from_stdin()?),
    }
}

/// Validates repository systemd units and writes a success message.
///
/// # Example
/// Valid checked-in units produce the appliance-contract confirmation line.
fn run_systemd_gate(out: &mut impl io::Write) -> io::Result<()> {
    run_systemd_gate_with(
        out,
        repovec_core::appliance::systemd_units::validate_checked_in_systemd_units,
    )
}

/// Runs the supplied validator before writing the systemd gate confirmation.
///
/// # Example
/// A validator returning `Ok(())` writes the confirmation; an error writes none.
fn run_systemd_gate_with<F>(out: &mut impl io::Write, validator: F) -> io::Result<()>
where
    F: FnOnce() -> Result<(), repovec_core::appliance::systemd_units::SystemdUnitError>,
{
    validator().map_err(io::Error::other)?;
    writeln!(out, "checked-in systemd units satisfy the appliance contract")?;
    out.flush()
}

/// Serializes documentation decisions as GitHub Actions key/value output.
///
/// # Example
/// A plan requiring documentation emits `docs_gate_required=true`.
fn write_plan(out: &mut impl io::Write, plan: &DocsGatePlan) -> io::Result<()> {
    writeln!(out, "should_run={}", plan.should_run())?;
    writeln!(out, "docs_gate_required={}", plan.docs_gate_required())?;
    writeln!(out, "nixie_required={}", plan.nixie_required())?;
    writeln!(out, "reason={}", plan.reason().as_str())?;
    writeln!(out, "matched_files_count={}", plan.matched_files().len())?;
    writeln!(out, "matched_files={}", plan.matched_files().join(","))?;
    writeln!(
        out,
        "conservative_fallback_files_count={}",
        plan.conservative_fallback_files().len()
    )?;
    writeln!(out, "conservative_fallback_files={}", plan.conservative_fallback_files().join(","))?;
    out.flush()
}

/// Selects a policy command, defaulting to documentation for an empty argument list.
///
/// # Example
/// `["systemd-gate"]` selects `Command::SystemdGate`; `[]` selects empty changed paths.
fn parse_args<I>(arguments: I) -> io::Result<Command>
where
    I: IntoIterator<Item = String>,
{
    let mut iter = arguments.into_iter();
    match iter.next() {
        Some(argument) if argument == "docs-gate" => {
            parse_docs_gate_args(iter).map(Command::DocsGate)
        }
        Some(argument) if argument == "systemd-gate" => parse_systemd_gate_args(iter),
        Some(argument) if argument == "--help" || argument == "-h" => Ok(Command::Help),
        Some(argument) => {
            parse_docs_gate_args(std::iter::once(argument).chain(iter)).map(Command::DocsGate)
        }
        None => Ok(Command::DocsGate(Input::ChangedFiles(Vec::new()))),
    }
}

/// Accepts systemd help or no extra arguments, rejecting other options.
///
/// # Example
/// `["--help"]` selects `Command::Help`; `["--stdin"]` returns an input error.
fn parse_systemd_gate_args<I>(arguments: I) -> io::Result<Command>
where
    I: IntoIterator<Item = String>,
{
    let mut iter = arguments.into_iter();
    match iter.next() {
        None => Ok(Command::SystemdGate),
        Some(argument) if argument == "--help" || argument == "-h" => Ok(Command::Help),
        Some(argument) => Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("unsupported argument for systemd-gate: {argument}\n\n{USAGE}"),
        )),
    }
}

/// Selects help, explicit paths or standard input, rejecting mixed input modes.
///
/// # Example
/// `["--changed-file", "README.md"]` selects that path; `["--stdin"]` selects stdin.
fn parse_docs_gate_args<I>(arguments: I) -> io::Result<Input>
where
    I: IntoIterator<Item = String>,
{
    let mut changed_files = Vec::new();
    let mut use_stdin = false;
    let mut iter = arguments.into_iter();

    while let Some(argument) = iter.next() {
        match argument.as_str() {
            "--help" | "-h" => return Ok(Input::Help),
            "--stdin" => use_stdin = true,
            "--changed-file" => {
                let path = iter.next().ok_or_else(|| {
                    io::Error::new(
                        io::ErrorKind::InvalidInput,
                        format!("missing value for --changed-file\n\n{USAGE}"),
                    )
                })?;
                changed_files.push(path);
            }
            _ => {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    format!("unsupported argument: {argument}\n\n{USAGE}"),
                ));
            }
        }
    }

    if use_stdin {
        if changed_files.is_empty() {
            Ok(Input::Stdin)
        } else {
            Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "--stdin cannot be combined with --changed-file",
            ))
        }
    } else {
        Ok(Input::ChangedFiles(changed_files))
    }
}

/// Collects each standard-input line as a changed path, propagating read failures.
///
/// # Example
/// Input `README.md` followed by `docs/guide.md` yields two paths in that order.
fn read_paths_from_stdin() -> io::Result<Vec<String>> {
    io::stdin().lock().lines().collect::<io::Result<Vec<_>>>()
}

#[cfg(test)]
#[path = "main_tests.rs"]
mod tests;
