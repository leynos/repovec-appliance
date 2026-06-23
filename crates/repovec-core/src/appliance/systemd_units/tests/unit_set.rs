//! Shared fixtures for mutating checked-in systemd units in validator tests.

use rstest::fixture;

use crate::appliance::systemd_units::{
    SystemdUnitError, checked_in_repovec_grepai_template, checked_in_repovec_mcpd_service,
    checked_in_repovec_target, checked_in_repovecd_service,
    validate_systemd_units_with_grepai_template,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum UnitFile {
    Target,
    Repovecd,
    Mcpd,
    GrepaiTemplate,
}

#[derive(Clone, Debug)]
pub(super) struct UnitSet {
    pub(super) target: String,
    pub(super) repovecd: String,
    pub(super) mcpd: String,
    pub(super) grepai_template: String,
}

impl UnitSet {
    pub(super) fn replace_file(&mut self, file: UnitFile, contents: &str) {
        *self.file_mut(file) = contents.to_owned();
    }

    pub(super) fn remove_line(&mut self, file: UnitFile, line: &str) {
        let contents = self.file_mut(file);
        let had_final_newline = contents.ends_with('\n');
        let line_to_remove = line.trim_end_matches(['\r', '\n']);
        let mut retained = contents
            .lines()
            .filter(|candidate| *candidate != line_to_remove)
            .collect::<Vec<_>>()
            .join("\n");

        if had_final_newline && !retained.is_empty() {
            retained.push('\n');
        }
        *contents = retained;
    }

    pub(super) fn replace_token(&mut self, file: UnitFile, from: &str, to: &str) {
        let contents = self.file_mut(file);
        *contents = contents.replace(from, to);
    }

    pub(super) fn remove_token(&mut self, file: UnitFile, key: &str, token: &str) {
        let contents = self.file_mut(file);
        let had_final_newline = contents.ends_with('\n');
        let mut lines = contents
            .lines()
            .map(|line| remove_token_from_line(line, key, token))
            .collect::<Vec<_>>()
            .join("\n");

        if had_final_newline && !lines.is_empty() {
            lines.push('\n');
        }
        *contents = lines;
    }

    pub(super) fn validate(&self) -> Result<(), SystemdUnitError> {
        validate_systemd_units_with_grepai_template(
            &self.target,
            &self.repovecd,
            &self.mcpd,
            &self.grepai_template,
        )
    }

    fn file_mut(&mut self, file: UnitFile) -> &mut String {
        match file {
            UnitFile::Target => &mut self.target,
            UnitFile::Repovecd => &mut self.repovecd,
            UnitFile::Mcpd => &mut self.mcpd,
            UnitFile::GrepaiTemplate => &mut self.grepai_template,
        }
    }
}

fn remove_token_from_line(line: &str, key: &str, token: &str) -> String {
    let Some(value) = line.strip_prefix(key) else {
        return line.to_owned();
    };
    if !value.split_whitespace().any(|candidate| candidate == token) {
        return line.to_owned();
    }

    let retained = value.split_whitespace().filter(|candidate| *candidate != token);

    format!("{key}{}", retained.collect::<Vec<_>>().join(" "))
}

#[fixture]
pub(super) fn checked_in_unit_set() -> UnitSet {
    UnitSet {
        target: checked_in_repovec_target().to_owned(),
        repovecd: checked_in_repovecd_service().to_owned(),
        mcpd: checked_in_repovec_mcpd_service().to_owned(),
        grepai_template: checked_in_repovec_grepai_template().to_owned(),
    }
}
