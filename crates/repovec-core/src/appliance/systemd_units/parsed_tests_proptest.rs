//! Property-based tests for systemd unit parser invariants.

use proptest::{prelude::*, test_runner::TestCaseError};

use super::ParsedUnit;
use crate::appliance::systemd_units::SystemdUnitError;

const UNIT: &str = "property-test.service";

proptest! {
    /// Verifies that arbitrary horizontal whitespace around directive keys and
    /// values is normalized before service directive matching.
    #[test]
    fn service_directive_matching_normalizes_whitespace(
        (key, expected) in service_directive_pair(),
        key_left in whitespace(),
        key_right in whitespace(),
        value_left in whitespace(),
        value_right in whitespace(),
    ) {
        let contents = format!(
            "[Service]\n{key_left}{key}{key_right}={value_left}{expected}{value_right}\n",
        );
        let parsed = parse_valid(&contents)?;

        prop_assert_eq!(parsed.require_service_directive(key, expected), Ok(()));
    }

    /// Verifies that dependency lookup tokenizes every repeated directive value
    /// with `split_whitespace`, so spaces and tabs between units are equivalent.
    #[test]
    fn dependency_matching_tokenizes_repeated_multi_value_directives(
        prefix_tokens in prop::collection::vec(dependency_token(), 0..=3),
        suffix_tokens in prop::collection::vec(dependency_token(), 0..=3),
        first_separator in nonempty_whitespace(),
        second_separator in nonempty_whitespace(),
        leading in whitespace(),
        trailing in whitespace(),
    ) {
        let mut second_line_tokens = suffix_tokens;
        second_line_tokens.insert(0, "qdrant.service");
        let first_line = prefix_tokens.join(&first_separator);
        let second_line = second_line_tokens.join(&second_separator);
        let contents = format!(
            "[Unit]\nRequires={first_line}\nRequires={leading}{second_line}{trailing}\n",
        );
        let parsed = parse_valid(&contents)?;

        prop_assert_eq!(
            parsed.require_dependency("Unit", "Requires", "qdrant.service"),
            Ok(())
        );
    }

    /// Verifies that dependency matching is scoped to the requested
    /// section/key pair and still finds the generated dependency token in any
    /// supported systemd directive shape.
    #[test]
    fn dependency_matching_is_scoped_to_generated_section_key_pairs(
        (section, key, dependency) in dependency_directive_pair(),
        before in dependency_token(),
        after in dependency_token(),
        separator in nonempty_whitespace(),
    ) {
        let contents = format!(
            "[{section}]\n{key}={before}{separator}{dependency}{separator}{after}\n",
        );
        let parsed = parse_valid(&contents)?;

        prop_assert_eq!(parsed.require_dependency(section, key, dependency), Ok(()));
        let is_wrong_section_missing = matches!(
            parsed.require_dependency("Service", key, dependency),
            Err(SystemdUnitError::MissingDependency { section: "Service", .. })
        );
        prop_assert!(is_wrong_section_missing);
    }

    /// Verifies that service directive matching requires exactly one matching
    /// value under `[Service]`; the same key/value under other sections and
    /// duplicate service values are rejected.
    #[test]
    fn service_directive_matching_requires_single_service_value(
        (key, expected) in service_directive_pair(),
        other_section in non_service_section(),
    ) {
        let wrong_section_contents = format!("[{other_section}]\n{key}={expected}\n");
        let wrong_section = parse_valid(&wrong_section_contents)?;
        let is_wrong_section_rejected = matches!(
            wrong_section.require_service_directive(key, expected),
            Err(SystemdUnitError::IncorrectServiceDirective { actual, .. }) if actual.is_empty()
        );
        prop_assert!(is_wrong_section_rejected);

        let duplicate_contents = format!(
            "[Service]\n{key}={expected}\n{key}={expected}\n",
        );
        let duplicate = parse_valid(&duplicate_contents)?;
        let is_duplicate_rejected = matches!(
            duplicate.require_service_directive(key, expected),
            Err(SystemdUnitError::IncorrectServiceDirective { actual, .. })
                if actual == format!("{expected},{expected}")
        );
        prop_assert!(is_duplicate_rejected);
    }
}

fn parse_valid(contents: &str) -> Result<ParsedUnit, TestCaseError> {
    ParsedUnit::parse(UNIT, contents)
        .map_err(|error| TestCaseError::fail(format!("valid generated unit should parse: {error}")))
}

fn whitespace() -> impl Strategy<Value = String> { r"[ \t]{0,4}" }

fn nonempty_whitespace() -> impl Strategy<Value = String> { r"[ \t]{1,4}" }

fn dependency_token() -> impl Strategy<Value = &'static str> {
    prop::sample::select(vec![
        "repovecd.service",
        "repovec-mcpd.service",
        "cloudflared.service",
        "repovec.target",
    ])
}

fn dependency_directive_pair() -> impl Strategy<Value = (&'static str, &'static str, &'static str)>
{
    prop::sample::select(vec![
        ("Unit", "Requires", "qdrant.service"),
        ("Unit", "After", "qdrant.service"),
        ("Unit", "Wants", "repovecd.service"),
        ("Unit", "PartOf", "repovec.target"),
        ("Install", "WantedBy", "repovec.target"),
    ])
}

fn service_directive_pair() -> impl Strategy<Value = (&'static str, &'static str)> {
    prop::sample::select(vec![
        ("Type", "exec"),
        ("User", "repovec"),
        ("Group", "repovec"),
        ("WorkingDirectory", "/var/lib/repovec/worktrees/%I"),
        ("Restart", "on-failure"),
        ("RestartSec", "5s"),
        ("StandardOutput", "journal"),
        ("StandardError", "journal"),
    ])
}

fn non_service_section() -> impl Strategy<Value = &'static str> {
    prop::sample::select(vec!["Unit", "Install", "Socket"])
}
