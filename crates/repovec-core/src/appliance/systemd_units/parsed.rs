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

#[derive(Debug)]
pub(super) struct ParsedUnit {
    unit: &'static str,
    sections: BTreeMap<String, BTreeMap<String, Vec<String>>>,
}

enum ParsedLine<'line> {
    Section(&'line str),
    KeyValue(KeyValueLine<'line>),
    Ignored,
}

struct KeyValueLine<'line> {
    key: &'line str,
    value: &'line str,
}

struct LineContext<'line> {
    unit: &'static str,
    line_number: usize,
    line: &'line str,
}

impl ParsedUnit {
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

    pub(super) fn require_section(&self, section: &'static str) -> Result<(), SystemdUnitError> {
        if self.sections.contains_key(section) {
            Ok(())
        } else {
            Err(SystemdUnitError::MissingSection { unit: self.unit, section })
        }
    }

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

    fn directive_tokens(&self, section: &str, key: &str) -> BTreeSet<String> {
        self.values(section, key)
            .iter()
            .flat_map(|value| value.split_whitespace())
            .map(ToOwned::to_owned)
            .collect()
    }

    fn values(&self, section: &str, key: &str) -> &[String] {
        self.sections.get(section).and_then(|entries| entries.get(key)).map_or(&[], Vec::as_slice)
    }
}

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

fn parse_section_header(line: &str) -> Option<&str> { line.strip_prefix('[')?.strip_suffix(']') }

fn is_ignored_line(line: &str) -> bool { line.is_empty() || line.starts_with(['#', ';']) }

fn is_qdrant_quadlet_source(value: &str) -> bool {
    value == QDRANT_CONTAINER || value == QDRANT_CONTAINER_SERVICE
}
