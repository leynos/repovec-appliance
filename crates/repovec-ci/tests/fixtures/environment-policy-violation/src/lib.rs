//! Fixture that calls every method the environment-access policy forbids.
//!
//! `environment_policy_lint_ui` lints this package with the repository's
//! `clippy.toml` and asserts that each call is reported with its remedy.
//! Nothing else compiles it, and it is not a workspace member.

use std::ffi::OsString;

/// Read a variable as UTF-8.
pub fn read(name: &str) -> Option<String> { std::env::var(name).ok() }

/// Read a variable as an OS string.
pub fn read_os(name: &str) -> Option<OsString> { std::env::var_os(name) }

/// Enumerate the environment as UTF-8 pairs.
pub fn enumerate() -> Vec<(String, String)> { std::env::vars().collect() }

/// Enumerate the environment as OS-string pairs.
pub fn enumerate_os() -> Vec<(OsString, OsString)> { std::env::vars_os().collect() }

/// Set a variable in the current process.
///
/// # Safety
///
/// The caller must ensure no other thread is reading the environment.
pub unsafe fn set(name: &str, value: &str) { unsafe { std::env::set_var(name, value) } }

/// Remove a variable from the current process.
///
/// # Safety
///
/// The caller must ensure no other thread is reading the environment.
pub unsafe fn remove(name: &str) { unsafe { std::env::remove_var(name) } }
