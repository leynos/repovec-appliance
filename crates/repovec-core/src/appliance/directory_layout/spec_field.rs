//! Field-by-field comparison of a parsed tmpfiles entry against a directory spec.

use camino::Utf8PathBuf;

use super::{DirectoryLayoutError, DirectorySpec, Mode, parser};

/// One field of a [`DirectorySpec`], expressed as data.
///
/// The mode, owner, and group comparisons share one decision—does the entry satisfy the
/// contract, and if not, what the contract required against what the asset declares—and differ
/// only in the predicate and the error variant. Holding them together runs the three as one
/// ordered pass rather than three near-identical functions.
#[derive(Clone, Copy)]
enum SpecField {
    /// The required owning user.
    Owner,
    /// The required owning group.
    Group,
    /// The required permission bits.
    Mode,
}

impl SpecField {
    /// Whether `entry` satisfies this field of `spec`.
    fn is_satisfied(
        self,
        spec: &DirectorySpec,
        entry: &parser::TmpfilesEntry,
        actual_mode: Mode,
    ) -> bool {
        match self {
            Self::Owner => entry.user == spec.owner,
            Self::Group => entry.group == spec.group,
            Self::Mode => actual_mode == spec.mode,
        }
    }

    /// The violation raised when this field of `spec` is not satisfied.
    fn violation(
        self,
        spec: &DirectorySpec,
        entry: &parser::TmpfilesEntry,
        actual_mode: Mode,
    ) -> DirectoryLayoutError {
        let path = Utf8PathBuf::from(entry.path.as_str());
        match self {
            Self::Owner => DirectoryLayoutError::IncorrectOwner {
                path,
                expected: spec.owner,
                actual: entry.user.clone(),
            },
            Self::Group => DirectoryLayoutError::IncorrectGroup {
                path,
                expected: spec.group,
                actual: entry.group.clone(),
            },
            Self::Mode => DirectoryLayoutError::IncorrectMode {
                path,
                expected: spec.mode,
                actual: actual_mode,
            },
        }
    }
}

/// The compared fields, in the order their violations are reported.
///
/// The order is the contract: the validator returns the first unsatisfied field,
/// so the list runs from the least specific mismatch to the most.
const COMPARED_FIELDS: [SpecField; 3] = [SpecField::Owner, SpecField::Group, SpecField::Mode];

/// Compares a parsed entry against a directory spec.
///
/// `actual_mode` is the mode column already parsed by [`super::compare_spec`], which keeps the
/// mode comparison total: the explicitness and syntax failures have been reported before this
/// runs.
pub(super) fn compare_spec_against(
    spec: &DirectorySpec,
    entry: &parser::TmpfilesEntry,
    actual_mode: Mode,
) -> Result<(), DirectoryLayoutError> {
    COMPARED_FIELDS.into_iter().try_for_each(|field| {
        if field.is_satisfied(spec, entry, actual_mode) {
            Ok(())
        } else {
            Err(field.violation(spec, entry, actual_mode))
        }
    })
}
