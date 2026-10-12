//! Shared mutation fixtures for the directory-layout contract tests.
//!
//! Both the typed-error table in `tests.rs` and the `Display` snapshots in
//! `tests_display.rs` drive the validator by rewriting the checked-in assets
//! into a violating state. The mutation model and its helpers live here so the
//! two test modules stay about *what* is asserted rather than *how* the
//! violating input is built.

use crate::appliance::directory_layout::{
    DirectoryLayoutError, checked_in_repovec_sysusers, checked_in_repovec_tmpfiles,
    validate_directory_layout,
};

/// A mutation that rewrites the checked-in asset pair into a violating state.
///
/// The mutation is expressed as a pair of string replacements so the test stays
/// readable; each case mutates exactly one of the two assets.
#[derive(Clone, Copy, Debug)]
pub(super) struct Mutation {
    tmpfiles_from: Option<&'static str>,
    tmpfiles_to: &'static str,
    sysusers_from: Option<&'static str>,
    sysusers_to: &'static str,
}

impl Mutation {
    pub(super) const fn tmpfiles(from: &'static str, to: &'static str) -> Self {
        Self { tmpfiles_from: Some(from), tmpfiles_to: to, sysusers_from: None, sysusers_to: "" }
    }

    pub(super) const fn sysusers(from: &'static str, to: &'static str) -> Self {
        Self { tmpfiles_from: None, tmpfiles_to: "", sysusers_from: Some(from), sysusers_to: to }
    }

    fn apply(self, tmpfiles: &mut String, sysusers: &mut String) {
        if let Some(from) = self.tmpfiles_from {
            *tmpfiles = tmpfiles.replacen(from, self.tmpfiles_to, 1);
        }
        if let Some(from) = self.sysusers_from {
            *sysusers = sysusers.replacen(from, self.sysusers_to, 1);
        }
    }

    /// Applies the mutation to the checked-in assets and returns the pair.
    pub(super) fn apply_to_checked_in(self) -> (String, String) {
        let mut tmpfiles = checked_in_repovec_tmpfiles().to_owned();
        let mut sysusers = checked_in_repovec_sysusers().to_owned();
        self.apply(&mut tmpfiles, &mut sysusers);
        (tmpfiles, sysusers)
    }
}

/// Returns the unmutated checked-in asset pair.
pub(super) fn checked_in() -> (String, String) {
    (checked_in_repovec_tmpfiles().to_owned(), checked_in_repovec_sysusers().to_owned())
}

/// Asserts that applying `mutation` yields exactly `expected`.
pub(super) fn assert_mutation(mutation: Mutation, expected: DirectoryLayoutError) {
    let (tmpfiles, sysusers) = mutation.apply_to_checked_in();

    let result = validate_directory_layout(&tmpfiles, &sysusers);

    assert_eq!(result, Err(expected));
}

/// Applies `mutation` and returns the error the validator reported.
pub(super) fn validate_mutation(mutation: Mutation) -> DirectoryLayoutError {
    let (tmpfiles, sysusers) = mutation.apply_to_checked_in();
    match validate_directory_layout(&tmpfiles, &sysusers) {
        Err(err) => err,
        Ok(()) => panic!("expected a validation failure"),
    }
}
