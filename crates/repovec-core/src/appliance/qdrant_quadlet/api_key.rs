//! API-key-specific validation for the Qdrant Quadlet contract.

use tracing::warn;

use super::{
    CONTAINER_SECTION, LOG_TARGET, QDRANT_API_KEY_ENVIRONMENT_VARIABLE, QDRANT_API_KEY_SECRET,
    QDRANT_API_KEY_SERVICE, QdrantQuadletError, UNIT_SECTION, parser::ParsedQuadlet,
};

pub(super) fn validate_api_key_provisioning_dependency(
    parsed: &ParsedQuadlet,
) -> Result<(), QdrantQuadletError> {
    validate_unit_dependency(parsed, "Requires")?;
    validate_unit_dependency(parsed, "After")
}

fn validate_unit_dependency(
    parsed: &ParsedQuadlet,
    directive: &'static str,
) -> Result<(), QdrantQuadletError> {
    let dependencies = parsed.values(UNIT_SECTION, directive);
    if dependencies.is_empty() {
        warn!(
            target: LOG_TARGET,
            directive,
            expected_dependency = QDRANT_API_KEY_SERVICE,
            "qdrant quadlet validation failed: missing api key provisioning dependency"
        );
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
    warn!(
        target: LOG_TARGET,
        directive,
        dependency,
        expected_dependency = QDRANT_API_KEY_SERVICE,
        "qdrant quadlet validation failed: incorrect api key provisioning dependency"
    );
    Err(QdrantQuadletError::IncorrectApiKeyProvisioningDependency { directive, dependency })
}

pub(super) fn validate_api_key_secret(parsed: &ParsedQuadlet) -> Result<(), QdrantQuadletError> {
    let secrets = parsed.values(CONTAINER_SECTION, "Secret");
    if secrets.is_empty() {
        warn!(
            target: LOG_TARGET,
            expected_secret = QDRANT_API_KEY_SECRET,
            expected_target = QDRANT_API_KEY_ENVIRONMENT_VARIABLE,
            "qdrant quadlet validation failed: missing api key secret"
        );
        return Err(QdrantQuadletError::MissingApiKeySecret);
    }

    if secrets.iter().any(|secret| is_required_api_key_secret(secret)) {
        return Ok(());
    }

    let secret = secrets.join(",");
    warn!(
        target: LOG_TARGET,
        secret,
        expected_secret = QDRANT_API_KEY_SECRET,
        expected_target = QDRANT_API_KEY_ENVIRONMENT_VARIABLE,
        "qdrant quadlet validation failed: incorrect api key secret"
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
        has_env_type |= key == "type" && value == "env";
        has_target |= key == "target" && value == QDRANT_API_KEY_ENVIRONMENT_VARIABLE;
    }

    has_env_type && has_target
}

pub(super) fn validate_no_inline_api_key_environment(
    parsed: &ParsedQuadlet,
) -> Result<(), QdrantQuadletError> {
    for environment in parsed.values(CONTAINER_SECTION, "Environment") {
        for assignment in split_environment_assignments(environment) {
            if is_api_key_environment_assignment(&assignment) {
                let redacted_environment = redact_api_key_environment_assignment(&assignment);
                warn!(
                    target: LOG_TARGET,
                    environment = redacted_environment,
                    expected_secret = QDRANT_API_KEY_SECRET,
                    expected_target = QDRANT_API_KEY_ENVIRONMENT_VARIABLE,
                    "qdrant quadlet validation failed: inline api key environment is disallowed"
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
    assignment
        .split_once('=')
        .map_or_else(|| assignment.to_owned(), |(key, _)| format!("{key}=<redacted>"))
}

fn split_environment_assignments(environment: &str) -> Vec<String> {
    let mut assignments = Vec::new();
    let mut assignment = String::new();
    let mut quote = None;

    for character in environment.chars() {
        match (quote, character) {
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
