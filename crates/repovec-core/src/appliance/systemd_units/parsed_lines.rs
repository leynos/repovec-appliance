//! Line-level parsing for the static systemd unit validator.
//!
//! This module classifies trimmed unit lines and inserts parsed directives into
//! the section map owned by [`super::ParsedUnit`]. It deliberately knows
//! nothing about which directives a unit contract requires.

use std::collections::BTreeMap;

use super::SystemdUnitError;

/// A significant line classified by the static unit parser.
pub(super) enum ParsedLine<'line> {
    /// A section header, such as `[Service]`.
    Section(&'line str),
    /// A directive's key and value, such as `User=repovec`.
    KeyValue(KeyValueLine<'line>),
    /// A blank line or a comment that contributes no directive.
    Ignored,
}

/// Borrowed key and value slices from one directive line.
pub(super) struct KeyValueLine<'line> {
    /// Directive name before the first equals sign.
    key: &'line str,
    /// Directive value after the first equals sign.
    value: &'line str,
}

/// Source location and unit identity used to report a malformed directive.
pub(super) struct LineContext<'line> {
    /// Unit name associated with the source line.
    pub(super) unit: &'static str,
    /// One-based line number in the unit text.
    pub(super) line_number: usize,
    /// Trimmed source line being processed.
    pub(super) line: &'line str,
}

/// Classifies a trimmed line as ignored, a section, or a key/value directive.
///
/// For example, `[Unit]` becomes a section and `After=network.target` a directive; malformed
/// non-comment text returns `Err(())` for the caller to attach unit and line context.
pub(super) fn parse_line(line: &str) -> Result<ParsedLine<'_>, ()> {
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
pub(super) fn insert_key_value(
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
