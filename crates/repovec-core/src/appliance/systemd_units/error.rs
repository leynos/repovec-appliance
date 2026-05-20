//! Semantic validation errors for the repovec systemd unit contract.
//!
//! The parent `systemd_units` module returns these errors from its checked-in
//! and caller-provided validation functions so callers can distinguish parse,
//! dependency, install-contract, and service command failures without
//! inspecting display strings.

use std::{error::Error, fmt};

/// Contract failures for the repovec systemd unit set.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SystemdUnitError {
    /// A unit file could not be parsed as a small subset of INI syntax.
    InvalidLine {
        /// The logical unit file name.
        unit: &'static str,
        /// The 1-indexed source line number.
        line_number: usize,
        /// The invalid line contents after trimming.
        line: String,
    },
    /// A key-value pair appeared before any section header.
    PropertyBeforeSection {
        /// The logical unit file name.
        unit: &'static str,
        /// The 1-indexed source line number.
        line_number: usize,
        /// The misplaced property line after trimming.
        line: String,
    },
    /// A required section is missing from a unit file.
    MissingSection {
        /// The logical unit file name.
        unit: &'static str,
        /// The required section name without brackets.
        section: &'static str,
    },
    /// A required dependency token is missing from a systemd directive.
    MissingDependency {
        /// The logical unit file name.
        unit: &'static str,
        /// The section containing the directive.
        section: &'static str,
        /// The directive name.
        key: &'static str,
        /// The required systemd unit dependency.
        dependency: &'static str,
    },
    /// A dependency references the source Quadlet name instead of the generated service.
    UsesQuadletSourceDependency {
        /// The logical unit file name.
        unit: &'static str,
        /// The section containing the directive.
        section: &'static str,
        /// The directive name.
        key: &'static str,
        /// The invalid dependency name.
        dependency: String,
    },
    /// A required service command is absent or points to the wrong binary.
    IncorrectExecStart {
        /// The logical unit file name.
        unit: &'static str,
        /// The expected executable path.
        expected: &'static str,
        /// The observed executable paths joined by commas.
        actual: String,
    },
    /// A required `[Service]` identity directive is absent or has the wrong value.
    IncorrectServiceDirective {
        /// The logical unit file name.
        unit: &'static str,
        /// The directive key (e.g. `"User"`).
        key: &'static str,
        /// The required value.
        expected: &'static str,
        /// The observed value(s) joined by commas, or an empty string if absent.
        actual: String,
    },
}

impl SystemdUnitError {
    /// Returns the logical systemd unit name associated with this validation error.
    #[must_use]
    pub const fn unit(&self) -> &str {
        match self {
            Self::InvalidLine { unit, .. }
            | Self::PropertyBeforeSection { unit, .. }
            | Self::MissingSection { unit, .. }
            | Self::MissingDependency { unit, .. }
            | Self::UsesQuadletSourceDependency { unit, .. }
            | Self::IncorrectExecStart { unit, .. }
            | Self::IncorrectServiceDirective { unit, .. } => unit,
        }
    }
}

impl fmt::Display for SystemdUnitError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidLine { unit, line_number, line } => {
                write!(f, "invalid systemd line in {unit} at {line_number}: {line}")
            }
            Self::PropertyBeforeSection { unit, line_number, line } => {
                write!(f, "systemd property before section in {unit} on line {line_number}: {line}")
            }
            Self::MissingSection { unit, section } => {
                write!(f, "{unit} is missing [{section}]")
            }
            Self::MissingDependency { unit, section, key, dependency } => {
                write!(f, "{unit} is missing {key}={dependency} in [{section}]")
            }
            Self::UsesQuadletSourceDependency { unit, section, key, dependency } => write!(
                f,
                "{unit} must depend on qdrant.service, not {dependency}, in [{section}] {key}",
            ),
            Self::IncorrectExecStart { unit, expected, actual } => {
                write!(f, "{unit} must use ExecStart={expected}: {actual}")
            }
            Self::IncorrectServiceDirective { unit, key, expected, actual } => {
                write!(f, "{unit} must have {key}={expected} in [Service]: {actual}")
            }
        }
    }
}

impl Error for SystemdUnitError {}

#[cfg(test)]
mod tests {
    //! Unit coverage for systemd unit validation errors.

    use super::SystemdUnitError;

    #[test]
    fn unit_returns_logical_unit_name() {
        let cases = [
            SystemdUnitError::InvalidLine {
                unit: "repovec.target",
                line_number: 1,
                line: "not valid".to_owned(),
            },
            SystemdUnitError::PropertyBeforeSection {
                unit: "repovecd.service",
                line_number: 1,
                line: "User=repovec".to_owned(),
            },
            SystemdUnitError::MissingSection { unit: "repovec-mcpd.service", section: "Service" },
            SystemdUnitError::MissingDependency {
                unit: "repovec.target",
                section: "Unit",
                key: "Wants",
                dependency: "qdrant.service",
            },
            SystemdUnitError::UsesQuadletSourceDependency {
                unit: "repovecd.service",
                section: "Unit",
                key: "Requires",
                dependency: "qdrant.container".to_owned(),
            },
            SystemdUnitError::IncorrectExecStart {
                unit: "repovecd.service",
                expected: "/usr/bin/repovecd",
                actual: "/usr/bin/wrong-binary".to_owned(),
            },
            SystemdUnitError::IncorrectServiceDirective {
                unit: "repovec-mcpd.service",
                key: "User",
                expected: "repovec",
                actual: "root".to_owned(),
            },
        ];

        let observed_units = cases.map(|error| error.unit().to_owned());

        assert_eq!(
            observed_units,
            [
                "repovec.target",
                "repovecd.service",
                "repovec-mcpd.service",
                "repovec.target",
                "repovecd.service",
                "repovecd.service",
                "repovec-mcpd.service",
            ],
        );
    }
}
