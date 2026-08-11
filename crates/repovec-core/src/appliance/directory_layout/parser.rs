//! Low-level tokenizer for the `tmpfiles.d` and `sysusers.d` assets.
//!
//! The tokenizer handles comments, blank lines, the `-` placeholder token, and
//! the quoted GECOS field used by the `sysusers.d` declaration. It yields
//! whitespace-separated columns while preserving quoted phrases as a single
//! column, which keeps the column indices correct for the sysusers home and
//! shell fields even when the GECOS comment contains spaces.
//!
//! The views are strict: [`tmpfiles_entry`] accepts only the `d` type this
//! asset uses and rejects anything else with [`MalformedLine`]. The tokenizer
//! itself never panics and never returns a partially populated view.

use crate::appliance::directory_layout::DirectoryLayoutError;

/// Splits a trimmed, non-comment line into shell-style quoted-aware columns.
fn tokenize(line: &str) -> Vec<String> {
    let mut columns = Vec::new();
    let mut current = String::new();
    let mut in_quotes = false;
    let mut chars = line.chars().peekable();

    while let Some(ch) = chars.next() {
        match ch {
            '"' => {
                in_quotes = !in_quotes;
                if !in_quotes && !current.is_empty() {
                    columns.push(std::mem::take(&mut current));
                }
            }
            c if c.is_whitespace() && !in_quotes => {
                if !current.is_empty() {
                    columns.push(std::mem::take(&mut current));
                }
            }
            c => current.push(c),
        }
    }

    if !current.is_empty() {
        columns.push(current);
    }
    columns
}

/// A parsed `d` entry from the `tmpfiles.d` asset.
pub(crate) struct TmpfilesEntry {
    pub kind: String,
    pub path: String,
    pub mode: String,
    pub user: String,
    pub group: String,
}

/// Parses an arbitrary line as a `tmpfiles.d` `d` entry.
pub(crate) fn tmpfiles_entry(
    line: &str,
    asset: &'static str,
    line_number: usize,
) -> Result<Option<TmpfilesEntry>, DirectoryLayoutError> {
    let trimmed = line.trim();
    if trimmed.is_empty() || trimmed.starts_with('#') {
        return Ok(None);
    }

    let columns = tokenize(trimmed);
    if columns.len() != 6 || columns[0] != "d" {
        return Err(DirectoryLayoutError::MalformedLine {
            asset,
            line_number,
            line: trimmed.to_owned(),
        });
    }

    Ok(Some(TmpfilesEntry {
        kind: columns[0].clone(),
        path: columns[1].clone(),
        mode: columns[2].clone(),
        user: columns[3].clone(),
        group: columns[4].clone(),
    }))
}

/// A parsed `u` line from the `sysusers.d` asset.
pub(crate) struct SysusersUserLine {
    pub name: String,
    pub home: String,
    pub shell: String,
}

/// Parses an arbitrary line as a `sysusers.d` `u` user declaration.
pub(crate) fn sysusers_user_line(
    line: &str,
    asset: &'static str,
    line_number: usize,
) -> Result<Option<SysusersUserLine>, DirectoryLayoutError> {
    let trimmed = line.trim();
    if trimmed.is_empty() || trimmed.starts_with('#') {
        return Ok(None);
    }

    let columns = tokenize(trimmed);
    if columns.len() != 6 || columns[0] != "u" {
        return Err(DirectoryLayoutError::MalformedLine {
            asset,
            line_number,
            line: trimmed.to_owned(),
        });
    }

    Ok(Some(SysusersUserLine {
        name: columns[1].clone(),
        home: columns[4].clone(),
        shell: columns[5].clone(),
    }))
}
