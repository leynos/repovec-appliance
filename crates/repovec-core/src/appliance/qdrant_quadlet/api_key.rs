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
/// ```rust,no_run
/// use repovec_core::appliance::qdrant_quadlet::{
///     QdrantQuadletError, checked_in_qdrant_quadlet, validate_qdrant_quadlet,
/// };
///
/// # fn main() -> Result<(), QdrantQuadletError> {
/// validate_qdrant_quadlet(checked_in_qdrant_quadlet(), &())?;
/// # Ok(())
/// # }
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
/// ```rust,no_run
/// use repovec_core::appliance::qdrant_quadlet::{
///     checked_in_qdrant_quadlet, validate_qdrant_quadlet,
/// };
///
/// assert!(validate_qdrant_quadlet(checked_in_qdrant_quadlet(), &()).is_ok());
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
/// ```rust,no_run
/// use repovec_core::appliance::qdrant_quadlet::{
///     QdrantQuadletError, validate_qdrant_quadlet,
/// };
///
/// let contents = concat!(
///     "[Unit]\n",
///     "Requires=repovec-qdrant-api-key.service\n",
///     "After=repovec-qdrant-api-key.service\n",
///     "\n",
///     "[Container]\n",
///     "Image=docker.io/qdrant/qdrant:v1\n",
///     "AutoUpdate=registry\n",
///     "Secret=repovec-qdrant-api-key,type=env,target=QDRANT__SERVICE__API_KEY\n",
///     "Environment=QDRANT__SERVICE__API_KEY=secret\n",
///     "PublishPort=127.0.0.1:6333:6333\n",
///     "PublishPort=127.0.0.1:6334:6334\n",
///     "Volume=/var/lib/repovec/qdrant-storage:/qdrant/storage:Z\n",
/// );
///
/// assert!(matches!(
///     validate_qdrant_quadlet(contents, &()),
///     Err(QdrantQuadletError::InlineApiKeyEnvironmentDisallowed { .. })
/// ));
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

pub(super) fn redact_api_key_environment_assignment(assignment: &str) -> String {
    match assignment.split_once('=') {
        Some((key, _)) => format!("{key}=<redacted>"),
        None => assignment.to_owned(),
    }
}

pub(super) fn split_environment_assignments(environment: &str) -> Vec<String> {
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
            (None, '"' | '\'') if assignment.is_empty() || assignment.ends_with('=') => {
                quote = Some(character);
            }
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

pub(super) fn is_api_key_environment_assignment(assignment: &str) -> bool {
    assignment == QDRANT_API_KEY_ENVIRONMENT_VARIABLE
        || assignment
            .split_once('=')
            .is_some_and(|(key, _value)| key == QDRANT_API_KEY_ENVIRONMENT_VARIABLE)
}

#[cfg(test)]
mod proptests {
    //! Property tests for API-key environment assignment parsing.

    use proptest::prelude::*;

    use super::{
        QDRANT_API_KEY_ENVIRONMENT_VARIABLE, is_api_key_environment_assignment,
        redact_api_key_environment_assignment, split_environment_assignments,
    };

    const REDACTED_API_KEY: &str = "QDRANT__SERVICE__API_KEY=<redacted>";

    #[derive(Clone, Copy, Debug)]
    enum QuoteStyle {
        Bare,
        Single,
        Double,
    }

    fn quote_style_strategy() -> impl Strategy<Value = QuoteStyle> {
        prop_oneof![Just(QuoteStyle::Bare), Just(QuoteStyle::Single), Just(QuoteStyle::Double)]
    }

    fn assignment(key: &str, value: &str, quote_style: QuoteStyle) -> (String, String) {
        match quote_style {
            QuoteStyle::Bare => {
                let bare_value = value.replace(' ', "_");
                (format!("{key}={bare_value}"), format!("{key}={bare_value}"))
            }
            QuoteStyle::Single => (format!("{key}='{value}'"), format!("{key}={value}")),
            QuoteStyle::Double => (format!(r#"{key}="{value}""#), format!("{key}={value}")),
        }
    }

    prop_compose! {
        fn neighbouring_assignment()(
            key in "[A-Z][A-Z0-9_]{0,8}",
            value in "zz[0-9A-F _-]{1,18}",
            quote_style in quote_style_strategy(),
        ) -> String {
            assignment(&key, &value, quote_style).0
        }
    }

    prop_compose! {
        fn api_key_assignment()(
            value in "zz[0-9A-F _-]{1,18}",
            quote_style in quote_style_strategy(),
        ) -> (String, String, String) {
            let (rendered, token) =
                assignment(QDRANT_API_KEY_ENVIRONMENT_VARIABLE, &value, quote_style);
            let payload = token
                .split_once('=')
                .map_or_else(String::new, |(_key, assignment_value)| {
                    assignment_value.to_owned()
                });
            (rendered, token, payload)
        }
    }

    proptest! {
        #[test]
        fn api_key_assignment_survives_splitting(
            before in proptest::collection::vec(neighbouring_assignment(), 1..4),
            api_key in api_key_assignment(),
            mut after in proptest::collection::vec(neighbouring_assignment(), 0..3),
            separator in "[ \t]{1,4}",
        ) {
            after.push(String::from("BROKEN='unterminated value"));
            let (rendered_api_key, expected_api_key, _payload) = api_key;
            let mut rendered = before;
            rendered.push(rendered_api_key);
            rendered.extend(after);

            let tokens = split_environment_assignments(&rendered.join(&separator));

            prop_assert!(tokens.contains(&expected_api_key));
            let api_key_is_isolated = tokens.iter().all(|token| {
                token == &expected_api_key || !token.contains(QDRANT_API_KEY_ENVIRONMENT_VARIABLE)
            });
            prop_assert!(api_key_is_isolated);
        }

        #[test]
        fn api_key_assignment_is_detected(value in "zz[0-9A-F_./:-]{1,32}") {
            let assignment = format!("{QDRANT_API_KEY_ENVIRONMENT_VARIABLE}={value}");

            prop_assert!(is_api_key_environment_assignment(&assignment));
            prop_assert!(is_api_key_environment_assignment(QDRANT_API_KEY_ENVIRONMENT_VARIABLE));
        }

        #[test]
        fn api_key_assignment_is_redacted(value in "zz[0-9A-F _-]{1,32}") {
            let assignment = format!("{QDRANT_API_KEY_ENVIRONMENT_VARIABLE}={value}");
            let redacted = redact_api_key_environment_assignment(&assignment);

            prop_assert_eq!(redacted.as_str(), REDACTED_API_KEY);
            prop_assert!(!redacted.contains(&value));
        }

        #[test]
        fn split_detect_redact_pipeline_holds(
            before in proptest::collection::vec(neighbouring_assignment(), 0..3),
            api_key in prop_oneof![
                api_key_assignment().prop_map(Some),
                Just(None),
            ],
            after in proptest::collection::vec(neighbouring_assignment(), 0..3),
            separator in "[ \t]{1,4}",
        ) {
            let mut rendered_assignments = before;
            let expected_payload = api_key.as_ref().map(|(_rendered, _token, payload)| payload.clone());
            rendered_assignments.push(api_key.map_or_else(
                || QDRANT_API_KEY_ENVIRONMENT_VARIABLE.to_owned(),
                |(rendered_assignment, _token, _payload)| rendered_assignment,
            ));
            rendered_assignments.extend(after);

            for token in split_environment_assignments(&rendered_assignments.join(&separator)) {
                if is_api_key_environment_assignment(&token) {
                    let redacted = redact_api_key_environment_assignment(&token);
                    if token == QDRANT_API_KEY_ENVIRONMENT_VARIABLE {
                        prop_assert_eq!(redacted, QDRANT_API_KEY_ENVIRONMENT_VARIABLE);
                    } else {
                        prop_assert_eq!(redacted.as_str(), REDACTED_API_KEY);
                        if let Some(payload) = &expected_payload {
                            prop_assert!(!redacted.contains(payload));
                        }
                    }
                }
            }
        }
    }
}
