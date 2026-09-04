//! Property-based robustness tests for the directory-layout asset tokenizer.

use proptest::prelude::*;

use super::parser::{sysusers_user_line, tmpfiles_entry};
use crate::appliance::directory_layout::DirectoryLayoutError;

const TMPFILES_ASSET: &str = "tmpfiles.d/repovec.conf";
const SYSUSERS_ASSET: &str = "sysusers.d/repovec.conf";

proptest! {
    /// For an arbitrary line the tokenizer-backed views either yield a
    /// well-formed entry or return `MalformedLine`. They never panic and never
    /// return a partially populated entry (no field filtering of inputs the
    /// parser must handle).
    #[test]
    fn arbitrarily_shaped_lines_never_panic_and_never_partially_populate(
        line in r"[^\n]{0,80}",
    ) {
        let tmpfiles = tmpfiles_entry(&line, TMPFILES_ASSET, 1);
        match tmpfiles {
            Ok(None) | Err(DirectoryLayoutError::MalformedLine { .. }) => {}
            Ok(Some(entry)) => {
                prop_assert!(!entry.path.is_empty(), "path must be populated");
                prop_assert!(!entry.mode.is_empty(), "mode must be populated");
                prop_assert!(!entry.user.is_empty(), "user must be populated");
                prop_assert!(!entry.group.is_empty(), "group must be populated");
            }
            Err(other) => {
                // The tmpfiles view must only ever surface MalformedLine.
                panic!("unexpected error variant: {other}");
            }
        }

        let sysusers = sysusers_user_line(&line, SYSUSERS_ASSET, 1);
        match sysusers {
            Ok(None) | Err(DirectoryLayoutError::MalformedLine { .. }) => {}
            Ok(Some(entry)) => {
                prop_assert!(!entry.name.is_empty(), "name must be populated");
                prop_assert!(!entry.home.is_empty(), "home must be populated");
                prop_assert!(!entry.shell.is_empty(), "shell must be populated");
            }
            Err(other) => {
                panic!("unexpected error variant: {other}");
            }
        }
    }
}
