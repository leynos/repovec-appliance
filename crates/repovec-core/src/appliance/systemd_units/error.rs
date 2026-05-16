//! Semantic validation errors for the repovec systemd unit contract.

use std::{error::Error, fmt};

/// Contract failures for the checked-in repovec systemd units.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SystemdUnitError {
    /// A unit line could not be parsed as minimal INI syntax.
    InvalidLine {
        /// The 1-indexed source line number.
        line_number: usize,
        /// The invalid line after trimming.
        line: String,
    },
    /// A key-value pair appeared before any section header.
    PropertyBeforeSection {
        /// The 1-indexed source line number.
        line_number: usize,
        /// The misplaced property line after trimming.
        line: String,
    },
    /// A required directive is absent.
    MissingDirective {
        /// The systemd unit being validated.
        unit: &'static str,
        /// The required section name.
        section: &'static str,
        /// The required directive name.
        directive: &'static str,
    },
    /// A service dependency directive does not include the expected unit.
    MissingDependency {
        /// The systemd unit being validated.
        unit: &'static str,
        /// The dependency directive name.
        directive: &'static str,
        /// The expected dependency unit.
        dependency: &'static str,
    },
    /// A directive has an unexpected value.
    IncorrectDirective {
        /// The systemd unit being validated.
        unit: &'static str,
        /// The directive name.
        directive: &'static str,
        /// The expected directive value.
        expected: &'static str,
        /// The observed directive value.
        actual: String,
    },
    /// A service unit has the wrong `ExecStart=` command.
    IncorrectExecStart {
        /// The systemd unit being validated.
        unit: &'static str,
        /// The expected command.
        expected: &'static str,
        /// The observed command.
        actual: String,
    },
}

impl fmt::Display for SystemdUnitError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidLine { line_number, line } => {
                write!(formatter, "invalid systemd unit line {line_number}: {line}")
            }
            Self::PropertyBeforeSection { line_number, line } => {
                write!(
                    formatter,
                    "systemd unit property before section on line {line_number}: {line}"
                )
            }
            Self::MissingDirective { unit, section, directive } => {
                write!(formatter, "{unit} is missing {directive}= in [{section}]")
            }
            Self::MissingDependency { unit, directive, dependency } => {
                write!(formatter, "{unit} {directive}= must include {dependency}")
            }
            Self::IncorrectDirective { unit, directive, expected, actual } => {
                write!(formatter, "{unit} {directive}= must be {expected}: {actual}")
            }
            Self::IncorrectExecStart { unit, expected, actual } => {
                write!(formatter, "{unit} ExecStart= must be {expected}: {actual}")
            }
        }
    }
}

impl Error for SystemdUnitError {}
