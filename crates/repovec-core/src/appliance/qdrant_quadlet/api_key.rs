//! API-key-specific validation for the Qdrant Quadlet contract.
//!
//! This module enforces the API-key boundaries of the Qdrant Quadlet contract:
//! provisioning dependency checks that `Requires=` and `After=` reference the
//! API key provisioning service, secret wiring checks that `Secret=` entries
//! use the expected secret name with `type=env` and the target environment
//! variable, and inline environment prohibition that forbids assigning the API
//! key environment variable directly.
//!
//! These validators are invoked from `validate_qdrant_quadlet` after the
//! structural contract checks for image, ports, storage, and auto-update have
//! completed.

use super::{
    CONTAINER_SECTION, QDRANT_API_KEY_ENVIRONMENT_VARIABLE, QDRANT_API_KEY_SECRET,
    QDRANT_API_KEY_SERVICE, QdrantQuadletError, UNIT_SECTION, observer::QdrantQuadletObserver,
    parser::ParsedQuadlet,
};

/// Validates that the parsed Quadlet declares the API key provisioning dependency.
///
/// # Errors
///
/// Returns [`QdrantQuadletError::MissingApiKeyProvisioningDependency`] when a
/// required dependency directive is absent, or
/// [`QdrantQuadletError::IncorrectApiKeyProvisioningDependency`] when the
/// directive does not reference the API key provisioning service.
///
/// # Examples
///
/// ```ignore
/// let parsed = ParsedQuadlet::parse(
///     "[Unit]\nRequires=repovec-qdrant-api-key.service\nAfter=repovec-qdrant-api-key.service\n",
///     &(),
/// )?;
///
/// validate_api_key_provisioning_dependency(&parsed, &())?;
/// # Ok::<(), QdrantQuadletError>(())
/// ```
pub(super) fn validate_api_key_provisioning_dependency(
    parsed: &ParsedQuadlet,
    observer: &dyn QdrantQuadletObserver,
) -> Result<(), QdrantQuadletError> {
    validate_unit_dependency(parsed, "Requires", observer)?;
    validate_unit_dependency(parsed, "After", observer)
}

fn validate_unit_dependency(
    parsed: &ParsedQuadlet,
    directive: &'static str,
    observer: &dyn QdrantQuadletObserver,
) -> Result<(), QdrantQuadletError> {
    let dependencies = parsed.values(UNIT_SECTION, directive);
    if dependencies.is_empty() {
        observer.missing_api_key_provisioning_dependency(directive, QDRANT_API_KEY_SERVICE);
        return Err(QdrantQuadletError::MissingApiKeyProvisioningDependency { directive });
    }

    if dependencies
        .iter()
        .flat_map(|dependency| dependency.split_ascii_whitespace())
        .any(|dependency| dependency == QDRANT_API_KEY_SERVICE)
    {
        return Ok(());
    }

    let dependency = dependencies.join(",");
    observer.incorrect_api_key_provisioning_dependency(
        directive,
        &dependency,
        QDRANT_API_KEY_SERVICE,
    );
    Err(QdrantQuadletError::IncorrectApiKeyProvisioningDependency { directive, dependency })
}

/// Validates that the parsed Quadlet wires the API key through a Podman secret.
///
/// The expected `Secret=` entry is
/// `repovec-qdrant-api-key,type=env,target=QDRANT__SERVICE__API_KEY`.
///
/// # Errors
///
/// Returns [`QdrantQuadletError::MissingApiKeySecret`] when no `Secret=` entry
/// exists, or [`QdrantQuadletError::IncorrectApiKeySecret`] when none of the
/// entries match the expected secret name, type, and target environment
/// variable.
///
/// # Examples
///
/// ```ignore
/// let parsed = ParsedQuadlet::parse(
///     "[Container]\nSecret=repovec-qdrant-api-key,type=env,target=QDRANT__SERVICE__API_KEY\n",
///     &(),
/// )?;
///
/// assert!(validate_api_key_secret(&parsed, &()).is_ok());
/// # Ok::<(), QdrantQuadletError>(())
/// ```
pub(super) fn validate_api_key_secret(
    parsed: &ParsedQuadlet,
    observer: &dyn QdrantQuadletObserver,
) -> Result<(), QdrantQuadletError> {
    let secrets = parsed.values(CONTAINER_SECTION, "Secret");
    if secrets.is_empty() {
        observer.missing_api_key_secret(QDRANT_API_KEY_SECRET, QDRANT_API_KEY_ENVIRONMENT_VARIABLE);
        return Err(QdrantQuadletError::MissingApiKeySecret);
    }

    if secrets.iter().any(|secret| is_required_api_key_secret(secret)) {
        return Ok(());
    }

    let secret = secrets.join(",");
    observer.incorrect_api_key_secret(
        &secret,
        QDRANT_API_KEY_SECRET,
        QDRANT_API_KEY_ENVIRONMENT_VARIABLE,
    );
    Err(QdrantQuadletError::IncorrectApiKeySecret { secret })
}

fn is_required_api_key_secret(secret: &str) -> bool {
    let mut parts = secret.split(',');
    if parts.next() != Some(QDRANT_API_KEY_SECRET) {
        return false;
    }

    let mut has_env_type = false;
    let mut has_target = false;
    for part in parts {
        let Some((key, value)) = part.split_once('=') else {
            continue;
        };
        match key {
            "type" if value == "env" => has_env_type = true,
            "target" if value == QDRANT_API_KEY_ENVIRONMENT_VARIABLE => has_target = true,
            "type" | "target" => return false,
            _ => {}
        }
    }

    has_env_type && has_target
}

/// Validates that no inline `QDRANT__SERVICE__API_KEY` assignments exist.
///
/// The check scans `[Container]` `Environment=` entries so the API key is only
/// supplied through the expected Podman secret wiring.
///
/// # Errors
///
/// Returns [`QdrantQuadletError::InlineApiKeyEnvironmentDisallowed`] with a
/// redacted assignment when an inline API key environment assignment is found.
///
/// # Examples
///
/// ```ignore
/// let parsed = ParsedQuadlet::parse(
///     "[Container]\nEnvironment=QDRANT__SERVICE__API_KEY=secret\n",
///     &(),
/// )?;
///
/// assert!(matches!(
///     validate_no_inline_api_key_environment(&parsed, &()),
///     Err(QdrantQuadletError::InlineApiKeyEnvironmentDisallowed { .. })
/// ));
/// # Ok::<(), QdrantQuadletError>(())
/// ```
pub(super) fn validate_no_inline_api_key_environment(
    parsed: &ParsedQuadlet,
    observer: &dyn QdrantQuadletObserver,
) -> Result<(), QdrantQuadletError> {
    for environment in parsed.values(CONTAINER_SECTION, "Environment") {
        for assignment in split_environment_assignments(environment) {
            if is_api_key_environment_assignment(&assignment) {
                let redacted_environment = redact_api_key_environment_assignment(&assignment);
                observer.inline_api_key_environment(
                    &redacted_environment,
                    QDRANT_API_KEY_SECRET,
                    QDRANT_API_KEY_ENVIRONMENT_VARIABLE,
                );
                return Err(QdrantQuadletError::InlineApiKeyEnvironmentDisallowed {
                    environment: redacted_environment,
                });
            }
        }
    }

    Ok(())
}

fn redact_api_key_environment_assignment(assignment: &str) -> String {
    match assignment.split_once('=') {
        Some((key, _)) => format!("{key}=<redacted>"),
        None => assignment.to_owned(),
    }
}

fn split_environment_assignments(environment: &str) -> Vec<String> {
    let mut assignments = Vec::new();
    let mut assignment = String::new();
    let mut quote = None;
    let mut is_escaped = false;

    // split_environment_assignments uses a quote-aware linear scan because
    // Quadlet Environment= values may contain spaces inside quoted KEY=VALUE
    // pairs. The quote state records when whitespace belongs to the current
    // assignment rather than separating assignments.
    //
    // Escaped characters inside quoted strings are kept in the current
    // assignment. Unmatched quotes are treated as part of the current assignment
    // until the scan ends. Empty assignments created by repeated whitespace are
    // skipped.
    for character in environment.chars() {
        if is_escaped {
            assignment.push(character);
            is_escaped = false;
            continue;
        }

        match (quote, character) {
            (Some(_), '\\') => {
                assignment.push(character);
                is_escaped = true;
            }
            (Some(active_quote), current) if active_quote == current => quote = None,
            (None, '"' | '\'') => quote = Some(character),
            (None, current) if current.is_ascii_whitespace() => {
                if !assignment.is_empty() {
                    assignments.push(std::mem::take(&mut assignment));
                }
            }
            _ => assignment.push(character),
        }
    }

    if !assignment.is_empty() {
        assignments.push(assignment);
    }

    assignments
}

fn is_api_key_environment_assignment(assignment: &str) -> bool {
    assignment == QDRANT_API_KEY_ENVIRONMENT_VARIABLE
        || assignment
            .split_once('=')
            .is_some_and(|(key, _value)| key == QDRANT_API_KEY_ENVIRONMENT_VARIABLE)
}

#[cfg(test)]
mod tests {
    //! Unit tests for API-key environment assignment tokenisation.

    use rstest::rstest;

    use super::{
        QDRANT_API_KEY_ENVIRONMENT_VARIABLE, is_api_key_environment_assignment,
        redact_api_key_environment_assignment, split_environment_assignments,
    };

    #[rstest]
    #[case::double_quoted_values_with_spaces(
        r#"FOO="hello world" BAR=baz"#,
        vec!["FOO=hello world", "BAR=baz"],
    )]
    #[case::single_quoted_values_with_spaces(
        "FOO='hello world' BAR=baz",
        vec!["FOO=hello world", "BAR=baz"],
    )]
    #[case::repeated_whitespace(
        "  FOO=bar \t  BAR=baz  ",
        vec!["FOO=bar", "BAR=baz"],
    )]
    #[case::unmatched_quote(
        r#"FOO="unterminated value BAR=baz"#,
        vec!["FOO=unterminated value BAR=baz"],
    )]
    #[case::escaped_quotes_inside_quoted_value(
        r#"FOO="hello \"quoted\" world" BAR=baz"#,
        vec![r#"FOO=hello \"quoted\" world"#, "BAR=baz"],
    )]
    fn split_environment_assignments_preserves_quote_aware_assignments(
        #[case] environment: &str,
        #[case] expected: Vec<&str>,
    ) {
        assert_eq!(split_environment_assignments(environment), expected);
    }

    #[test]
    fn is_api_key_environment_assignment_detects_bare_variable() {
        assert!(is_api_key_environment_assignment(QDRANT_API_KEY_ENVIRONMENT_VARIABLE));
    }

    #[test]
    fn redact_api_key_environment_assignment_formats_key_value_pair() {
        assert_eq!(
            redact_api_key_environment_assignment("QDRANT__SERVICE__API_KEY=secret"),
            "QDRANT__SERVICE__API_KEY=<redacted>",
        );
    }
}
