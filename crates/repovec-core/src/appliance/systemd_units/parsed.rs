//! Parsed representation for the static systemd unit validator.
//!
//! This module owns the small section-aware parser used by the parent
//! [`super`] module. [`ParsedUnit`] records a systemd unit file as ordered
//! sections with their key/value directives so the validator can ask semantic
//! questions such as whether a required section exists, whether a dependency
//! appears in the expected directive, or whether a service directive has the
//! exact shipped value.
//!
//! The parser intentionally covers only the syntax needed for checked-in
//! appliance units: section headers, `key=value` directives, comments, and
//! blank lines. It keeps live systemd interaction out of the validation path;
//! the parent module decides which directives belong to each unit contract.

use std::collections::{BTreeMap, BTreeSet};

use super::{QDRANT_CONTAINER, QDRANT_CONTAINER_SERVICE, SERVICE_SECTION, SystemdUnitError};

#[cfg(test)]
#[path = "parsed_tests_proptest.rs"]
mod tests_proptest;

/// Parsed systemd unit identity and its section directives.
#[derive(Debug)]
pub(super) struct ParsedUnit {
    /// Unit name included in validation errors.
    unit: &'static str,
    /// Ordered sections with each directive's values in source order.
    sections: BTreeMap<String, BTreeMap<String, Vec<String>>>,
}

/// A significant line classified by the static unit parser.
enum ParsedLine<'line> {
    /// A section header, such as `[Service]`.
    Section(&'line str),
    /// A directive's key and value, such as `User=repovec`.
    KeyValue(KeyValueLine<'line>),
    /// A blank line or a comment that contributes no directive.
    Ignored,
}

/// Borrowed key and value slices from one directive line.
struct KeyValueLine<'line> {
    /// Directive name before the first equals sign.
    key: &'line str,
    /// Directive value after the first equals sign.
    value: &'line str,
}

/// Source location and unit identity used to report a malformed directive.
struct LineContext<'line> {
    /// Unit name associated with the source line.
    unit: &'static str,
    /// One-based line number in the unit text.
    line_number: usize,
    /// Trimmed source line being processed.
    line: &'line str,
}

impl ParsedUnit {
    /// Parses section headers and directives, returning the first malformed-line error.
    ///
    /// For example, `parse("demo.service", "[Service]\nUser=repovec")` stores the service
    /// directive; a property before any section returns [`SystemdUnitError::PropertyBeforeSection`].
    pub(super) fn parse(unit: &'static str, contents: &str) -> Result<Self, SystemdUnitError> {
        let mut sections = BTreeMap::<String, BTreeMap<String, Vec<String>>>::new();
        let mut current_section: Option<&str> = None;

        for (line_index, raw_line) in contents.lines().enumerate() {
            let line_number = line_index + 1;
            let line = raw_line.trim();

            match parse_line(line).map_err(|()| SystemdUnitError::InvalidLine {
                unit,
                line_number,
                line: line.to_owned(),
            })? {
                ParsedLine::Ignored => {}
                ParsedLine::Section(section) => {
                    sections.entry(section.to_owned()).or_default();
                    current_section = Some(section);
                }
                ParsedLine::KeyValue(key_value) => {
                    let context = LineContext { unit, line_number, line };
                    insert_key_value(&context, current_section, &mut sections, &key_value)?;
                }
            }
        }

        Ok(Self { unit, sections })
    }

    /// Requires a named section; a missing `[Service]` yields [`SystemdUnitError::MissingSection`].
    pub(super) fn require_section(&self, section: &'static str) -> Result<(), SystemdUnitError> {
        if self.sections.contains_key(section) {
            Ok(())
        } else {
            Err(SystemdUnitError::MissingSection { unit: self.unit, section })
        }
    }

    /// Requires a dependency token and rejects Quadlet-derived Qdrant names first.
    ///
    /// For example, checking `After=qdrant.service` succeeds when that token is present; a
    /// `qdrant.container` token yields [`SystemdUnitError::UsesQuadletSourceDependency`].
    pub(super) fn require_dependency(
        &self,
        section: &'static str,
        key: &'static str,
        dependency: &'static str,
    ) -> Result<(), SystemdUnitError> {
        let tokens = self.directive_tokens(section, key);
        if let Some(quadlet_dependency) =
            tokens.iter().find(|value| is_qdrant_quadlet_source(value))
        {
            return Err(SystemdUnitError::UsesQuadletSourceDependency {
                unit: self.unit,
                section,
                key,
                dependency: quadlet_dependency.to_owned(),
            });
        }

        if tokens.contains(dependency) {
            return Ok(());
        }

        Err(SystemdUnitError::MissingDependency { unit: self.unit, section, key, dependency })
    }

    /// Requires exactly one `ExecStart` value equal to `expected`.
    ///
    /// For example, expected `/usr/bin/repovecd` succeeds only when the service has that single
    /// command; absent or additional values yield [`SystemdUnitError::IncorrectExecStart`].
    pub(super) fn require_exec_start(
        &self,
        expected: &'static str,
    ) -> Result<(), SystemdUnitError> {
        let values = self.values(SERVICE_SECTION, "ExecStart");
        if let [actual] = values
            && actual == expected
        {
            return Ok(());
        }

        Err(SystemdUnitError::IncorrectExecStart {
            unit: self.unit,
            expected,
            actual: values.join(","),
        })
    }

    /// Requires `ExecStart=` to declare exactly `expected`, in this order.
    ///
    /// Membership is not enough for the provisioning oneshot: `systemd-sysusers`
    /// must run before `systemd-tmpfiles`, and a repeated or extra command would
    /// otherwise pass validation. Comparing the whole sequence rejects a swapped
    /// order, a missing command, and an unexpected addition alike.
    pub(super) fn require_exec_start_sequence(
        &self,
        expected: &'static [&'static str],
    ) -> Result<(), SystemdUnitError> {
        let values = self.values(SERVICE_SECTION, "ExecStart");
        if values.iter().map(String::as_str).eq(expected.iter().copied()) {
            return Ok(());
        }

        Err(SystemdUnitError::IncorrectExecStartSequence {
            unit: self.unit,
            expected,
            actual: values.to_vec(),
        })
    }

    /// Requires one service directive with the exact expected value.
    ///
    /// For example, `User=repovec` satisfies a matching expectation; a different value yields
    /// [`SystemdUnitError::IncorrectServiceDirective`].
    pub(super) fn require_service_directive(
        &self,
        key: &'static str,
        expected: &'static str,
    ) -> Result<(), SystemdUnitError> {
        let values = self.values(SERVICE_SECTION, key);
        if let [actual] = values
            && actual == expected
        {
            return Ok(());
        }

        Err(SystemdUnitError::IncorrectServiceDirective {
            unit: self.unit,
            key,
            expected,
            actual: values.join(","),
        })
    }

    /// Requires the expected value among the service's `Environment` entries.
    ///
    /// For example, `HOME=/var/lib/repovec` succeeds when present; otherwise the directive error
    /// records the observed environment values.
    pub(super) fn require_service_environment(
        &self,
        expected: &'static str,
    ) -> Result<(), SystemdUnitError> {
        let values = self.values(SERVICE_SECTION, "Environment");
        if values.iter().any(|value| value == expected) {
            return Ok(());
        }

        Err(SystemdUnitError::IncorrectServiceDirective {
            unit: self.unit,
            key: "Environment",
            expected,
            actual: values.join(","),
        })
    }

    /// Splits all values for a directive into a set of whitespace-delimited tokens.
    ///
    /// For example, `After=qdrant.service repovecd.service` yields both dependency tokens.
    fn directive_tokens(&self, section: &str, key: &str) -> BTreeSet<String> {
        self.values(section, key)
            .iter()
            .flat_map(|value| value.split_whitespace())
            .map(ToOwned::to_owned)
            .collect()
    }

    /// Returns directive values in source order, or an empty slice when absent.
    ///
    /// For example, querying `("Service", "User")` returns the stored `User` values.
    fn values(&self, section: &str, key: &str) -> &[String] {
        self.sections.get(section).and_then(|entries| entries.get(key)).map_or(&[], Vec::as_slice)
    }
}

/// Classifies a trimmed line as ignored, a section, or a key/value directive.
///
/// For example, `[Unit]` becomes a section and `After=network.target` a directive; malformed
/// non-comment text returns `Err(())` for the caller to attach unit and line context.
fn parse_line(line: &str) -> Result<ParsedLine<'_>, ()> {
    if is_ignored_line(line) {
        return Ok(ParsedLine::Ignored);
    }

    if let Some(section) = parse_section_header(line) {
        return Ok(ParsedLine::Section(section));
    }

    let Some((key, value)) = line.split_once('=') else {
        return Err(());
    };

    Ok(ParsedLine::KeyValue(KeyValueLine { key, value }))
}

/// Adds a parsed directive to its section or reports a property before any section header.
///
/// For example, `User=repovec` is stored under the current `[Service]` section.
fn insert_key_value(
    context: &LineContext<'_>,
    current_section: Option<&str>,
    sections: &mut BTreeMap<String, BTreeMap<String, Vec<String>>>,
    key_value: &KeyValueLine<'_>,
) -> Result<(), SystemdUnitError> {
    let Some(section) = current_section else {
        return Err(SystemdUnitError::PropertyBeforeSection {
            unit: context.unit,
            line_number: context.line_number,
            line: context.line.to_owned(),
        });
    };

    sections
        .entry(section.to_owned())
        .or_default()
        .entry(key_value.key.trim().to_owned())
        .or_default()
        .push(key_value.value.trim().to_owned());

    Ok(())
}

/// Returns the borrowed section name for a bracketed header such as `[Install]`.
fn parse_section_header(line: &str) -> Option<&str> { line.strip_prefix('[')?.strip_suffix(']') }

/// Identifies blank lines and lines whose first character marks a systemd comment.
///
/// For example, an empty line or `# generated` returns `true`.
fn is_ignored_line(line: &str) -> bool { line.is_empty() || line.starts_with(['#', ';']) }

/// Recognizes Qdrant Quadlet source names that the unit contract forbids as dependencies.
///
/// For example, `qdrant.container` returns `true`, while `qdrant.service` returns `false`.
fn is_qdrant_quadlet_source(value: &str) -> bool {
    value == QDRANT_CONTAINER || value == QDRANT_CONTAINER_SERVICE
}
