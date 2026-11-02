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
    /// A required ordered sequence of service commands is absent, reordered, or
    /// extended with an unexpected command.
    IncorrectExecStartSequence {
        /// The logical unit file name.
        unit: &'static str,
        /// The expected commands, in the required order.
        expected: &'static [&'static str],
        /// The observed commands, in declaration order.
        actual: Vec<String>,
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
    /// A directive the contract forbids is present in the named section.
    ///
    /// Used where the defect is the directive's existence rather than its
    /// value: the provisioning oneshot must stay manually restartable, so any
    /// `[Unit] RefuseManualStop=` occurrence is rejected whatever it is set to.
    ForbiddenDirective {
        /// The logical unit file name.
        unit: &'static str,
        /// The section containing the forbidden directive.
        section: &'static str,
        /// The forbidden directive key (e.g. `"RefuseManualStop"`).
        key: &'static str,
        /// The observed value(s) joined by commas.
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
            | Self::IncorrectExecStartSequence { unit, .. }
            | Self::IncorrectServiceDirective { unit, .. }
            | Self::ForbiddenDirective { unit, .. } => unit,
        }
    }
}

impl fmt::Display for SystemdUnitError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidLine {
                unit,
                line_number,
                line,
            } => write_invalid_line(f, unit, *line_number, line),
            Self::PropertyBeforeSection {
                unit,
                line_number,
                line,
            } => write_property_before_section(f, unit, *line_number, line),
            Self::MissingSection { unit, section } => write_missing_section(f, unit, section),
            Self::MissingDependency {
                unit,
                section,
                key,
                dependency,
            } => write_missing_dependency(f, (unit, section, key, dependency)),
            Self::UsesQuadletSourceDependency {
                unit,
                section,
                key,
                dependency,
            } => write_uses_quadlet_source_dependency(f, (unit, section, key, dependency)),
            Self::IncorrectExecStart {
                unit,
                expected,
                actual,
            } => write_incorrect_exec_start(f, unit, expected, actual),
            Self::IncorrectExecStartSequence {
                unit,
                expected,
                actual,
            } => write_incorrect_exec_start_sequence(f, unit, expected, actual),
            Self::IncorrectServiceDirective {
                unit,
                key,
                expected,
                actual,
            } => write_incorrect_service_directive(f, (unit, key, expected, actual)),
            Self::ForbiddenDirective {
                unit,
                section,
                key,
                actual,
            } => write_forbidden_directive(f, (unit, section, key, actual)),
        }
    }
}

/// Writes the diagnostic for an unparsable unit line.
fn write_invalid_line(
    f: &mut fmt::Formatter<'_>,
    unit: &str,
    line_number: usize,
    line: &str,
) -> fmt::Result {
    write!(f, "invalid systemd line in {unit} at {line_number}: {line}")
}

/// Writes the diagnostic for a property that precedes any section header.
fn write_property_before_section(
    f: &mut fmt::Formatter<'_>,
    unit: &str,
    line_number: usize,
    line: &str,
) -> fmt::Result {
    write!(
        f,
        "systemd property before section in {unit} on line {line_number}: {line}"
    )
}

/// Writes the diagnostic for a missing required section.
fn write_missing_section(f: &mut fmt::Formatter<'_>, unit: &str, section: &str) -> fmt::Result {
    write!(f, "{unit} is missing [{section}]")
}

/// Writes the diagnostic for a missing dependency token.
///
/// This and the three dependency-shaped writers below take a destructured
/// tuple because the workspace's `too-many-arguments-threshold = 4` makes five
/// plain parameters a lint failure, and the tuple groups the diagnostic parts
/// without a struct that would exist only to carry them.
fn write_missing_dependency(
    f: &mut fmt::Formatter<'_>,
    (unit, section, key, dependency): (&str, &str, &str, &str),
) -> fmt::Result {
    write!(f, "{unit} is missing {key}={dependency} in [{section}]")
}

/// Writes the diagnostic for a dependency on the Quadlet source name.
fn write_uses_quadlet_source_dependency(
    f: &mut fmt::Formatter<'_>,
    (unit, section, key, dependency): (&str, &str, &str, &str),
) -> fmt::Result {
    write!(
        f,
        "{unit} must depend on qdrant.service, not {dependency}, in [{section}] {key}"
    )
}

/// Writes the diagnostic for a wrong `ExecStart=` command.
fn write_incorrect_exec_start(
    f: &mut fmt::Formatter<'_>,
    unit: &str,
    expected: &str,
    actual: &str,
) -> fmt::Result {
    write!(f, "{unit} must use ExecStart={expected}: {actual}")
}

/// Writes the diagnostic for a wrong `ExecStart=` command sequence.
fn write_incorrect_exec_start_sequence(
    f: &mut fmt::Formatter<'_>,
    unit: &str,
    expected: &[&str],
    actual: &[String],
) -> fmt::Result {
    write!(
        f,
        "{unit} must declare ExecStart={} in order: {}",
        expected.join(", "),
        actual.join(", ")
    )
}

/// Writes the diagnostic for a wrong `[Service]` identity directive.
fn write_incorrect_service_directive(
    f: &mut fmt::Formatter<'_>,
    (unit, key, expected, actual): (&str, &str, &str, &str),
) -> fmt::Result {
    write!(
        f,
        "{unit} must have {key}={expected} in [Service]: {actual}"
    )
}

/// Writes the diagnostic for a forbidden directive.
fn write_forbidden_directive(
    f: &mut fmt::Formatter<'_>,
    (unit, section, key, actual): (&str, &str, &str, &str),
) -> fmt::Result {
    write!(f, "{unit} must not declare {key} in [{section}]: {actual}")
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
            SystemdUnitError::MissingSection {
                unit: "repovec-mcpd.service",
                section: "Service",
            },
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
            SystemdUnitError::ForbiddenDirective {
                unit: "repovec-provision.service",
                section: "Unit",
                key: "RefuseManualStop",
                actual: "yes".to_owned(),
            },
            SystemdUnitError::MissingDependency {
                unit: "repovec-provision.service",
                section: "Unit",
                key: "Before",
                dependency: "repovecd.service",
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
                "repovec-provision.service",
                "repovec-provision.service",
            ],
        );
    }
}
