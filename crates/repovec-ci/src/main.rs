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

fn run() -> io::Result<()> {
    let mut stdout = BufWriter::new(io::stdout().lock());
    let command = parse_args(std::env::args().skip(1))?;
    match command {
        Command::Help => print_usage(&mut stdout),
        Command::DocsGate(input) => run_docs_gate(&mut stdout, input),
        Command::SystemdGate => run_systemd_gate(&mut stdout),
    }
}

enum Command {
    Help,
    DocsGate(Input),
    SystemdGate,
}

enum Input {
    Help,
    ChangedFiles(Vec<String>),
    Stdin,
}

fn print_usage(out: &mut impl io::Write) -> io::Result<()> {
    out.write_all(USAGE.as_bytes())?;
    out.flush()
}

fn evaluate_from_paths(paths: Vec<String>) -> io::Result<DocsGatePlan> {
    let root = Dir::open_ambient_dir(".", ambient_authority())?;
    Ok(evaluate_docs_gate_in(&root, paths))
}

fn evaluate_from_stdin() -> io::Result<DocsGatePlan> {
    let root = Dir::open_ambient_dir(".", ambient_authority())?;
    Ok(evaluate_docs_gate_in(&root, read_paths_from_stdin()?))
}

fn run_docs_gate(out: &mut impl io::Write, input: Input) -> io::Result<()> {
    match input {
        Input::Help => print_usage(out),
        Input::ChangedFiles(paths) => write_plan(out, &evaluate_from_paths(paths)?),
        Input::Stdin => write_plan(out, &evaluate_from_stdin()?),
    }
}

fn run_systemd_gate(out: &mut impl io::Write) -> io::Result<()> {
    run_systemd_gate_with(
        out,
        repovec_core::appliance::systemd_units::validate_checked_in_systemd_units,
    )
}

fn run_systemd_gate_with<F>(out: &mut impl io::Write, validator: F) -> io::Result<()>
where
    F: FnOnce() -> Result<(), repovec_core::appliance::systemd_units::SystemdUnitError>,
{
    validator().map_err(io::Error::other)?;
    writeln!(out, "checked-in systemd units satisfy the appliance contract")?;
    out.flush()
}

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

fn read_paths_from_stdin() -> io::Result<Vec<String>> {
    io::stdin().lock().lines().collect::<io::Result<Vec<_>>>()
}

#[cfg(test)]
#[path = "main_tests.rs"]
mod tests;
