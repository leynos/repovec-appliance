//! Minimal section-aware parser for the subset of systemd unit syntax used here.

use std::collections::BTreeMap;

use super::SystemdUnitError;

#[derive(Debug)]
pub(super) struct ParsedUnit {
    sections: BTreeMap<String, BTreeMap<String, Vec<String>>>,
}

impl ParsedUnit {
    pub(super) fn parse(contents: &str) -> Result<Self, SystemdUnitError> {
        let mut sections = BTreeMap::<String, BTreeMap<String, Vec<String>>>::new();
        let mut current_section = Option::<String>::None;

        for (line_index, raw_line) in contents.lines().enumerate() {
            let line_number = line_index + 1;
            let line = raw_line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }

            if let Some(section) = parse_section_header(line) {
                current_section = Some(section.to_owned());
                sections.entry(section.to_owned()).or_default();
                continue;
            }

            let Some((key, value)) = line.split_once('=') else {
                return Err(SystemdUnitError::InvalidLine { line_number, line: line.to_owned() });
            };

            let Some(section) = current_section.as_ref() else {
                return Err(SystemdUnitError::PropertyBeforeSection {
                    line_number,
                    line: line.to_owned(),
                });
            };

            sections
                .entry(section.clone())
                .or_default()
                .entry(key.trim().to_owned())
                .or_default()
                .push(value.trim().to_owned());
        }

        Ok(Self { sections })
    }

    pub(super) fn values(&self, section: &str, key: &str) -> &[String] {
        self.sections.get(section).and_then(|entries| entries.get(key)).map_or(&[], Vec::as_slice)
    }
}

fn parse_section_header(line: &str) -> Option<&str> { line.strip_prefix('[')?.strip_suffix(']') }
