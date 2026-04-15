//! Command-line entry point for CI policy helpers.

use std::io::{self, BufRead as _, BufWriter, Write as _};

use cap_std::{ambient_authority, fs_utf8::Dir};
use repovec_ci::{DocsGatePlan, evaluate_docs_gate_in};

const USAGE: &str = concat!(
    "Usage: repovec-ci [--changed-file <path> [--changed-file <path>]...] [--help]\n",
    "       repovec-ci --stdin\n\n",
    "Reads a changed-file list and prints documentation-gate decisions in\n",
    "GitHub Actions output format.\n"
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
    let input = parse_args(std::env::args().skip(1))?;
    match input {
        Input::Help => print_usage(&mut stdout),
        Input::ChangedFiles(paths) => write_plan(&mut stdout, &evaluate_from_paths(paths)?),
        Input::Stdin => write_plan(&mut stdout, &evaluate_from_stdin()?),
    }
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

fn parse_args<I>(arguments: I) -> io::Result<Input>
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
mod tests {
    //! Unit coverage for CLI argument parsing.

    use std::io;

    use insta::assert_snapshot;
    use repovec_ci::{MermaidDetection, evaluate_docs_gate_with};

    use super::{Input, USAGE, parse_args, print_usage, write_plan};

    fn buffer_to_string(buffer: Vec<u8>) -> String {
        String::from_utf8(buffer).expect("snapshot buffer should contain UTF-8")
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
        let input = parse_args(std::iter::empty::<String>()).expect("empty args should parse");

        match input {
            Input::ChangedFiles(paths) => assert!(paths.is_empty()),
            Input::Help | Input::Stdin => panic!("empty args should default to ChangedFiles"),
        }
    }

    #[test]
    fn help_flag_returns_help_input() {
        let input = parse_args(["--help".to_owned()]).expect("help flag should parse");

        assert!(matches!(input, Input::Help));
    }

    #[test]
    fn stdin_flag_returns_stdin_input() {
        let input = parse_args(["--stdin".to_owned()]).expect("stdin flag should parse");

        assert!(matches!(input, Input::Stdin));
    }

    #[test]
    fn changed_file_flags_collect_all_paths() {
        let input = parse_args([
            "--changed-file".to_owned(),
            "docs/users-guide.md".to_owned(),
            "--changed-file".to_owned(),
            ".markdownlint-cli2.jsonc".to_owned(),
        ])
        .expect("changed-file flags should parse");

        match input {
            Input::ChangedFiles(paths) => {
                assert_eq!(paths, vec!["docs/users-guide.md", ".markdownlint-cli2.jsonc"]);
            }
            Input::Help | Input::Stdin => panic!("changed-file flags should yield ChangedFiles"),
        }
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
}
