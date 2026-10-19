//! Property-based robustness tests for the directory-layout asset tokenizer.
//!
//! Two complementary properties live here. The round-trip properties generate
//! *structured* asset text, because free-form text essentially never reaches the
//! `Ok(Some(entry))` arm of either view and so would leave the happy path
//! unexercised. The free-form property then covers the opposite direction: for
//! arbitrary input the views must never panic and must never surface an error
//! variant outside the set they document.

use proptest::prelude::*;

use super::{
    error::{SYSUSERS_ASSET, TMPFILES_ASSET},
    parser::{sysusers_user_line, tmpfiles_entry},
};
use crate::appliance::directory_layout::DirectoryLayoutError;

/// Characters that can appear unquoted in an asset column without being
/// mistaken for a separator, a comment, or a quote.
const TOKEN_CHARS: &[char] = &['a', 'b', 'z', 'A', 'Z', '0', '9', '.', '_', '-'];

/// Characters safe inside a double-quoted GECOS field.
const GECOS_CHARS: &[char] = &['a', 'b', 'z', ' ', '.', '-'];

/// A single non-empty asset column built from unquoted-safe characters.
fn token(max_len: usize) -> impl Strategy<Value = String> {
    prop::collection::vec(prop::sample::select(TOKEN_CHARS), 1..=max_len)
        .prop_map(|chars| chars.into_iter().collect())
}

/// An absolute path under the data tree.
///
/// The leading `/` keeps the generated value from ever being exactly `-`, the
/// placeholder token, so the round trip stays unambiguous.
fn data_path() -> impl Strategy<Value = String> {
    token(12).prop_map(|name| format!("/var/lib/repovec/{name}"))
}

/// An octal mode column, three or four digits wide.
fn octal_mode() -> impl Strategy<Value = String> {
    prop::collection::vec(prop::sample::select(&['0', '1', '2', '3', '4', '5', '6', '7']), 3..=4)
        .prop_map(|digits| digits.into_iter().collect())
}

/// A quoted GECOS field, which may be empty and may contain spaces.
fn gecos() -> impl Strategy<Value = String> {
    prop::collection::vec(prop::sample::select(GECOS_CHARS), 0..=24)
        .prop_map(|chars| format!("\"{}\"", chars.into_iter().collect::<String>()))
}

/// The uid column of a `u` line, as written and as it must parse.
///
/// An empty quoted field is generated on purpose: the tokenizer must preserve
/// it as a column, because dropping it would shift the home and shell fields
/// left and silently mis-read a syntactically valid declaration. The two halves
/// of the pair differ for that case because a quoted field's value is the text
/// inside the quotes.
fn uid_column() -> impl Strategy<Value = (String, String)> {
    prop_oneof![
        Just(("-".to_owned(), "-".to_owned())),
        Just(("\"\"".to_owned(), String::new())),
        token(6).prop_map(|value| (value.clone(), value)),
    ]
}

proptest! {
    /// A well-formed `d` line round-trips through the view unchanged.
    #[test]
    fn well_formed_tmpfiles_lines_round_trip(
        path in data_path(),
        mode in octal_mode(),
        user in token(12),
        group in token(12),
    ) {
        let line = format!("d {path} {mode} {user} {group} -");
        let Some(entry) = tmpfiles_entry(&line, TMPFILES_ASSET, 1)? else {
            prop_assert!(false, "a well-formed d line must parse, got None for {line:?}");
            return Ok(());
        };

        prop_assert_eq!(&entry.path, &path);
        prop_assert_eq!(&entry.mode, &mode);
        prop_assert_eq!(&entry.user, &user);
        prop_assert_eq!(&entry.group, &group);
    }

    /// Runs of horizontal whitespace are a single column separator, so an
    /// asset written with tabs or aligned columns reads the same as one
    /// written with single spaces.
    ///
    /// The padding is `1..=4` rather than `0..=4`: with no separator at all the
    /// columns concatenate into one token, which is a genuinely malformed line
    /// rather than the well-formed one this property is about.
    #[test]
    fn tmpfiles_lines_tolerate_irregular_separators(
        path in data_path(),
        mode in octal_mode(),
        pad in "[ \t]{1,4}",
        user in token(12),
        group in token(12),
    ) {
        let line = format!("{pad}d{pad}{path}{pad}{mode}{pad}{user}{pad}{group}{pad}-{pad}");
        let Some(entry) = tmpfiles_entry(&line, TMPFILES_ASSET, 1)? else {
            prop_assert!(false, "a well-formed d line must parse, got None for {line:?}");
            return Ok(());
        };

        prop_assert_eq!(&entry.path, &path);
        prop_assert_eq!(&entry.mode, &mode);
        prop_assert_eq!(&entry.user, &user);
        prop_assert_eq!(&entry.group, &group);
    }

    /// A `u` line round-trips through the view unchanged, and the optional
    /// shell column is carried through rather than dropped.
    #[test]
    fn well_formed_sysusers_lines_round_trip(
        name in token(12),
        uid in uid_column(),
        gecos in gecos(),
        home in data_path(),
    ) {
        let (uid_written, uid_parsed) = uid;
        let line = format!("u {name} {uid_written} {gecos} {home} /usr/sbin/nologin");
        let Some(entry) = sysusers_user_line(&line, SYSUSERS_ASSET, 1)? else {
            prop_assert!(false, "a well-formed u line must parse, got None for {line:?}");
            return Ok(());
        };

        prop_assert_eq!(&entry.name, &name);
        prop_assert_eq!(&entry.uid, &uid_parsed);
        prop_assert_eq!(&entry.home, &home);
        prop_assert_eq!(&entry.shell, "/usr/sbin/nologin");
    }

    /// The `home` column is the fifth, not the fourth: an empty GECOS field
    /// must not shift it. This is the regression guard for quoted-field
    /// handling, expressed as a property so any future tokenizer change is
    /// checked across the whole generated name/home space.
    #[test]
    fn an_empty_gecos_does_not_shift_the_home_column(name in token(12), home in data_path()) {
        let line = format!("u {name} - \"\" {home} /usr/sbin/nologin");
        let Some(entry) = sysusers_user_line(&line, SYSUSERS_ASSET, 1)? else {
            prop_assert!(false, "a well-formed u line must parse, got None for {line:?}");
            return Ok(());
        };

        prop_assert_eq!(&entry.home, &home);
    }

    /// For arbitrary input the views never panic, and the error variants they
    /// surface stay within the set each view documents.
    #[test]
    fn arbitrarily_shaped_lines_never_panic(line in r"[^\n]{0,80}") {
        match tmpfiles_entry(&line, TMPFILES_ASSET, 1) {
            Ok(None | Some(_)) | Err(DirectoryLayoutError::MalformedLine { .. }) => {}
            Err(other) => prop_assert!(false, "tmpfiles view surfaced {other:?} for {line:?}"),
        }

        match sysusers_user_line(&line, SYSUSERS_ASSET, 1) {
            Ok(None | Some(_))
            | Err(
                DirectoryLayoutError::MalformedLine { .. }
                | DirectoryLayoutError::MissingSysusersField { .. },
            ) => {}
            Err(other) => prop_assert!(false, "sysusers view surfaced {other:?} for {line:?}"),
        }
    }
}
