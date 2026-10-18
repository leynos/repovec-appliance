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
    CONTAINER_SECTION, PROVISION_SERVICE, QDRANT_API_KEY_ENVIRONMENT_VARIABLE,
    QDRANT_API_KEY_SECRET, QDRANT_API_KEY_SERVICE, QdrantQuadletError, UNIT_SECTION,
    environment::{
        is_api_key_environment_assignment, redact_api_key_environment_assignment,
        split_environment_assignments,
    },
    observer::QdrantQuadletObserver,
    parser::ParsedQuadlet,
};

/// Validates that the parsed Quadlet declares the provisioning dependencies.
///
/// Two systemd units must be required, because either may fail and the
/// container must then fail closed rather than start against an unprovisioned
/// host:
///
/// - `repovec-qdrant-api-key.service`, which creates the Podman secret wired
///   through `Secret=`;
/// - `repovec-provision.service`, which creates the `repovec` account and the
///   `/var/lib/repovec/qdrant-storage` mount source.
///
/// `After=` is checked for the API-key service as well, because a `Requires=`
/// alone would not order the secret creation before container start.
///
/// # Errors
///
/// Returns [`QdrantQuadletError::MissingApiKeyProvisioningDependency`] or
/// [`QdrantQuadletError::IncorrectApiKeyProvisioningDependency`] for the
/// API-key oneshot, and [`QdrantQuadletError::MissingUnitDependency`] or
/// [`QdrantQuadletError::IncorrectUnitDependency`] for
/// `repovec-provision.service`. The API-key variants predate the general
/// dependency check and remain part of the public surface so existing callers
/// observing them keep working.
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
pub(super) fn validate_provisioning_dependency(
    parsed: &ParsedQuadlet,
    observer: &dyn QdrantQuadletObserver,
) -> Result<(), QdrantQuadletError> {
    for directive in REQUIRED_DEPENDENCY_DIRECTIVES {
        validate_unit_dependency(parsed, directive, QDRANT_API_KEY_SERVICE, observer)?;
    }

    validate_unit_dependency(parsed, "Requires", PROVISION_SERVICE, observer)
}

/// The directives that must reference `repovec-qdrant-api-key.service`.
///
/// `Requires=` makes a failed API-key provision cancel the container start;
/// `After=` orders the secret creation before the container tries to read it.
const REQUIRED_DEPENDENCY_DIRECTIVES: [&str; 2] = ["Requires", "After"];

/// Checks one systemd dependency directive for a required provisioning service.
///
/// The parsed Quadlet and directive name are inspected, and violations are
/// reported to the observer before returning a domain error. The general
/// callbacks forward API-key service events to their original callbacks by
/// default, while observers can override the general callbacks for both
/// dependencies.
/// For example, a `Requires` value containing
/// `repovec-qdrant-api-key.service` passes.
fn validate_unit_dependency(
    parsed: &ParsedQuadlet,
    directive: &'static str,
    expected: &'static str,
    observer: &dyn QdrantQuadletObserver,
) -> Result<(), QdrantQuadletError> {
    let dependencies = parsed.values(UNIT_SECTION, directive);
    if dependencies.is_empty() {
        observer.missing_provisioning_dependency(directive, expected);
        return Err(missing_dependency_error(directive, expected));
    }

    if dependencies
        .iter()
        .flat_map(|dependency| dependency.split_ascii_whitespace())
        .any(|dependency| dependency == expected)
    {
        return Ok(());
    }

    let dependency = dependencies.join(",");
    observer.incorrect_provisioning_dependency(directive, &dependency, expected);
    Err(incorrect_dependency_error(directive, expected, dependency))
}

/// Builds the missing-dependency error, preserving the API-key variant for the
/// oneshot that predates the general provisioning check.
fn missing_dependency_error(directive: &'static str, expected: &'static str) -> QdrantQuadletError {
    if expected == QDRANT_API_KEY_SERVICE {
        QdrantQuadletError::MissingApiKeyProvisioningDependency { directive }
    } else {
        QdrantQuadletError::MissingUnitDependency { directive, expected }
    }
}

/// Builds the wrong-dependency error, preserving the API-key variant for the
/// oneshot that predates the general provisioning check.
fn incorrect_dependency_error(
    directive: &'static str,
    expected: &'static str,
    dependency: String,
) -> QdrantQuadletError {
    if expected == QDRANT_API_KEY_SERVICE {
        QdrantQuadletError::IncorrectApiKeyProvisioningDependency { directive, dependency }
    } else {
        QdrantQuadletError::IncorrectUnitDependency { directive, expected, actual: dependency }
    }
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

/// Reports whether a `Secret=` value names the expected secret and environment target.
///
/// The comma-separated value must contain the required secret name,
/// `type=env`, and the API-key target. For example,
/// `repovec-qdrant-api-key,type=env,target=QDRANT__SERVICE__API_KEY` passes.
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
///     "Requires=repovec-qdrant-api-key.service repovec-provision.service\n",
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
