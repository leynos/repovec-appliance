//! Low-level tokenizer for the `tmpfiles.d` and `sysusers.d` assets.
//!
//! The tokenizer handles comments, blank lines, the `-` placeholder token, and
//! the quoted GECOS field used by the `sysusers.d` declaration. It yields
//! whitespace-separated columns while preserving quoted phrases as a single
//! column, which keeps the column indices correct for the sysusers home and
//! shell fields even when the GECOS comment contains spaces. A line that ends
//! inside a quoted section is rejected rather than partially consumed, matching
//! the `Syntax error.` that `systemd-sysusers` reports for the same input.
//!
//! The views accept optional fields where `systemd` permits them while still
//! enforcing the appliance policy. A sysusers `u` line permits one optional
//! shell field; persistent tmpfiles `d` lines permit only an omitted or `-`
//! cleanup age and an omitted or `-` unused argument. A short `u` line is
//! reported with the name of the first absent column, so the operator is told
//! which field to add rather than only that the line is malformed. The
//! tokenizer never panics and never returns a partially populated view.

use crate::appliance::directory_layout::{DirectoryLayoutError, Mode};

/// Quote-aware column accumulator for a single asset line.
///
/// The scan state—columns already emitted, the column under construction, and
/// whether the scan currently sits inside a quoted section—only makes sense
/// together, so it is held in one place rather than threaded through helper
/// functions as a parameter clump.
#[derive(Default)]
struct Tokenizer {
    columns: Vec<String>,
    current: String,
    in_quotes: bool,
}

impl Tokenizer {
    /// Closes the column under construction, preserving an empty quoted field.
    fn end_column(&mut self) { self.columns.push(std::mem::take(&mut self.current)); }

    /// Closes the current column only when it holds anything.
    ///
    /// Unquoted whitespace is skipped rather than emitted as an empty column,
    /// so the column indices of the fields after it stay correct.
    fn end_non_empty_column(&mut self) {
        if !self.current.is_empty() {
            self.end_column();
        }
    }

    /// Folds one character of the line into the tokenizer state.
    fn accept(&mut self, character: char) {
        match (self.in_quotes, character) {
            (true, '"') => {
                self.in_quotes = false;
                self.end_column();
            }
            (false, '"') => self.in_quotes = true,
            (false, current) if current.is_whitespace() => self.end_non_empty_column(),
            (_, current) => self.current.push(current),
        }
    }
}

/// Splits a trimmed, non-comment line into shell-style quoted-aware columns.
///
/// A quoted section always contributes a column, so an empty quoted field is
/// preserved rather than silently removed. Dropping it would shift every later
/// column and reject a syntactically valid declaration.
///
/// Returns `None` when the line ends inside a quoted section. `systemd` rejects
/// an unterminated quote outright—`systemd-sysusers` 257 prints
/// `Syntax error.`—so a tokenizer that instead absorbed the rest of the line
/// would hand the validator a silently truncated column list and let a typo
/// pass the static gate that is meant to catch it.
fn tokenize(line: &str) -> Option<Vec<String>> {
    let mut tokenizer = Tokenizer::default();
    for character in line.chars() {
        tokenizer.accept(character);
    }
    if tokenizer.in_quotes {
        return None;
    }
    tokenizer.end_non_empty_column();
    Some(tokenizer.columns)
}

/// Splits tokenized columns into the leading `N` interpreted columns and any
/// trailing ones.
///
/// Consuming the owned columns lets each view move its fields out instead of
/// cloning them, while still tolerating the optional trailing columns the
/// formats allow. Returns `None` when the line carries fewer than `N` columns.
fn split_columns<const N: usize>(mut columns: Vec<String>) -> Option<([String; N], Vec<String>)> {
    if columns.len() < N {
        return None;
    }
    let tail = columns.split_off(N);
    let head = columns.try_into().ok()?;
    Some((head, tail))
}

/// Reports whether an optional `d`-line age and argument preserve contents.
///
/// An omitted age or `-` disables time-based cleanup. The `d` type has no
/// argument, so systemd accepts only an omitted argument or its `-` placeholder.
fn tmpfiles_d_preserves_contents(tail: &[String]) -> bool {
    match tail {
        [] => true,
        [age] => age == "-",
        [age, argument] => age == "-" && argument == "-",
        _ => false,
    }
}

/// Builds the malformed-line error for a rejected asset line.
fn malformed_line(asset: &'static str, line_number: usize, trimmed: &str) -> DirectoryLayoutError {
    DirectoryLayoutError::MalformedLine { asset, line_number, line: trimmed.to_owned() }
}

/// A parsed `d` entry from the `tmpfiles.d` asset.
pub(crate) struct TmpfilesEntry {
    pub path: String,
    pub mode: String,
    pub user: String,
    pub group: String,
}

/// Parses an arbitrary line as a `tmpfiles.d` `d` entry.
///
/// Only the leading five columns describe the directory, but the optional age
/// and argument are checked because these paths hold persistent state. Cleanup
/// must be disabled: age is omitted or `-`, and the unused argument is omitted
/// or `-`.
pub(crate) fn tmpfiles_entry(
    line: &str,
    asset: &'static str,
    line_number: usize,
) -> Result<Option<TmpfilesEntry>, DirectoryLayoutError> {
    let trimmed = line.trim();
    if trimmed.is_empty() || trimmed.starts_with('#') {
        return Ok(None);
    }

    let Some(columns) = tokenize(trimmed) else {
        return Err(malformed_line(asset, line_number, trimmed));
    };
    let Some(([kind, path, mode, user, group], tail)) = split_columns(columns) else {
        return Err(malformed_line(asset, line_number, trimmed));
    };
    if kind != "d" || !tmpfiles_d_preserves_contents(&tail) {
        return Err(malformed_line(asset, line_number, trimmed));
    }

    Ok(Some(TmpfilesEntry { path, mode, user, group }))
}

/// A parsed `u` line from the `sysusers.d` asset.
pub(crate) struct SysusersUserLine {
    pub name: String,
    pub uid: String,
    pub home: String,
    pub shell: String,
}

/// The `u` columns this view interprets, in declaration order.
///
/// The name at index `n` is the `n`th column, so a short line can be reported
/// with the first column it is missing.
const SYSUSERS_COLUMNS: [&str; 5] = ["type", "name", "uid", "gecos", "home"];

/// Parses an arbitrary line as a `sysusers.d` `u` user declaration.
///
/// A `sysusers.d` `u` line is `u name uid gecos home shell`, and every column
/// after the type may be omitted. This view requires the five columns it
/// interprets and names the first absent one when the line is short; the
/// optional `shell` column is carried through when present.
pub(crate) fn sysusers_user_line(
    line: &str,
    asset: &'static str,
    line_number: usize,
) -> Result<Option<SysusersUserLine>, DirectoryLayoutError> {
    let trimmed = line.trim();
    if trimmed.is_empty() || trimmed.starts_with('#') {
        return Ok(None);
    }

    let Some(columns) = tokenize(trimmed) else {
        return Err(malformed_line(asset, line_number, trimmed));
    };
    if columns.first().is_none_or(|kind| kind != "u") {
        return Err(malformed_line(asset, line_number, trimmed));
    }
    let column_count = columns.len();
    let Some(([_kind, name, uid, _gecos, home], tail)) = split_columns(columns) else {
        // The line starts with `u` and has fewer than five columns, so the
        // first absent column is always addressable.
        let Some(field) = SYSUSERS_COLUMNS.get(column_count).copied() else {
            return Err(malformed_line(asset, line_number, trimmed));
        };
        return Err(DirectoryLayoutError::MissingSysusersField { asset, line_number, field });
    };
    if tail.len() > 1 {
        return Err(malformed_line(asset, line_number, trimmed));
    }

    Ok(Some(SysusersUserLine {
        name,
        uid,
        home,
        shell: tail.into_iter().next().unwrap_or_default(),
    }))
}

/// Why a mode column could not be accepted as an explicit octal mode.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ModeParseError {
    /// The `-` placeholder was used instead of an explicit value (SI-5).
    Placeholder,
    /// The field is explicit, but is not a valid octal mode.
    Invalid,
}

/// Parses an explicit octal mode string (e.g. `"0700"`).
///
/// # Errors
///
/// Returns [`ModeParseError::Placeholder`] for the `-` default token and
/// [`ModeParseError::Invalid`] for any other non-octal input. Keeping the two
/// apart lets the validator tell an operator that the mode is missing, rather
/// than wrongly reporting that a present but unparsable mode is missing.
pub(crate) fn parse_mode(text: &str) -> Result<Mode, ModeParseError> {
    if text == "-" {
        return Err(ModeParseError::Placeholder);
    }
    u16::from_str_radix(text, 8).map(Mode).map_err(|_| ModeParseError::Invalid)
}
