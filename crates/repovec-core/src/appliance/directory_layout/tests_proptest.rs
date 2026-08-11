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
            Ok(None) => {}
            Ok(Some(entry)) => {
                prop_assert_eq!(entry.kind, "d");
                prop_assert!(entry.mode.chars().all(|c| c.is_ascii_digit() || c == '-'));
            }
            Err(DirectoryLayoutError::MalformedLine { .. }) => {}
            Err(other) => {
                // The tmpfiles view must only ever surface MalformedLine.
                panic!("unexpected error variant: {other}");
            }
        }

        let sysusers = sysusers_user_line(&line, SYSUSERS_ASSET, 1);
        match sysusers {
            Ok(None) => {}
            Ok(Some(entry)) => {
                prop_assert_eq!(entry.name, "repovec");
                prop_assert!(!entry.home.is_empty());
                prop_assert!(!entry.shell.is_empty());
            }
            Err(DirectoryLayoutError::MalformedLine { .. }) => {}
            Err(other) => {
                panic!("unexpected error variant: {other}");
            }
        }
    }
}
