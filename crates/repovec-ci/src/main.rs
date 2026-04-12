//! Command-line entry point for CI policy helpers.

use std::io::{self, BufRead as _, BufWriter, Write as _};

use repovec_ci::evaluate_docs_gate;

fn main() {
    if let Err(error) = run() {
        match writeln!(io::stderr().lock(), "{error}") {
            Ok(()) | Err(_) => {}
        }
        std::process::exit(1);
    }
}

fn run() -> io::Result<()> {
    let input = parse_args(std::env::args().skip(1))?;
    let plan = match input {
        Input::ChangedFiles(paths) => evaluate_docs_gate(paths),
        Input::Stdin => evaluate_docs_gate(read_paths_from_stdin()?),
    };

    let mut stdout = BufWriter::new(io::stdout().lock());
    writeln!(stdout, "should_run={}", plan.should_run())?;
    writeln!(stdout, "docs_gate_required={}", plan.docs_gate_required())?;
    writeln!(stdout, "nixie_required={}", plan.nixie_required())?;
    writeln!(stdout, "reason={}", plan.reason().as_str())?;
    writeln!(stdout, "matched_count={}", plan.matched_files().len())?;
    writeln!(stdout, "matched_files={}", plan.matched_files().join(","))?;
    stdout.flush()
}

enum Input {
    ChangedFiles(Vec<String>),
    Stdin,
}

fn parse_args<I>(arguments: I) -> io::Result<Input>
where
    I: IntoIterator<Item = String>,
{
    let mut changed_files = Vec::new();
    let mut use_stdin = false;
    let mut iter = arguments.into_iter();

    while let Some(argument) = iter.next() {
        match argument.as_str() {
            "--stdin" => use_stdin = true,
            "--changed-file" => {
                let path = iter.next().ok_or_else(|| {
                    io::Error::new(io::ErrorKind::InvalidInput, "missing value for --changed-file")
                })?;
                changed_files.push(path);
            }
            _ => {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    format!("unsupported argument: {argument}"),
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
