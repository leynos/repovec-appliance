//! Provisioning-dependency log scenarios for the Qdrant Quadlet contract.
//!
//! A Qdrant container that starts without its provisioning oneshot will mount an
//! unprovisioned storage path and read a secret that was never created, so the
//! `[Unit]` dependency directives are part of the fail-closed contract. These
//! cases mutate those directives and pin the telemetry the validator must emit.

use tracing::Level;

use super::{
    LogScenario, canonical,
    log_test_case_support::{event, without_line},
};

/// The API-key service every `[Unit]` directive must name.
///
/// `Requires=` and `After=` are checked against the same unit, so the name is
/// shared by every scenario rather than repeated at each call site.
const API_KEY_SERVICE: &str = "repovec-qdrant-api-key.service";

/// A dependency directive that is present but names the wrong unit.
///
/// Grouping the mutation and the expected event fields keeps `incorrect_dependency`
/// within the argument-count budget while still naming each field at the call
/// site, so a reader can see what the Quadlet is mutated to and what the emitted
/// event must report.
#[derive(Clone, Copy)]
struct IncorrectDependencyCase {
    /// The `[Unit]` directive under test (`Requires` or `After`).
    directive: &'static str,
    /// The dependency the mutated Quadlet declares instead.
    dependency: &'static str,
    /// The dependency the contract requires the directive to name.
    expected_dependency: &'static str,
    /// The `directive=dependency` text the mutation searches for.
    from: &'static str,
    /// The replacement text.
    to: &'static str,
}

pub(super) fn missing_requires_dependency() -> LogScenario {
    missing_dependency(
        "missing_requires_dependency",
        "Requires=repovec-qdrant-api-key.service repovec-provision.service\n",
        "Requires",
        API_KEY_SERVICE,
    )
}

pub(super) fn missing_after_dependency() -> LogScenario {
    missing_dependency(
        "missing_after_dependency",
        "After=repovec-qdrant-api-key.service\n",
        "After",
        API_KEY_SERVICE,
    )
}

/// The `Requires=` line loses the provisioning oneshot but keeps the API-key
/// service, so the failure must name `repovec-provision.service` even though
/// the directive itself is still present.
pub(super) fn incorrect_provision_dependency() -> LogScenario {
    incorrect_dependency(
        "incorrect_provision_dependency",
        IncorrectDependencyCase {
            directive: "Requires",
            dependency: API_KEY_SERVICE,
            expected_dependency: "repovec-provision.service",
            from: "Requires=repovec-qdrant-api-key.service repovec-provision.service",
            to: "Requires=repovec-qdrant-api-key.service",
        },
    )
}

fn missing_dependency(
    snapshot_name: &'static str,
    line: &str,
    directive: &'static str,
    expected_dependency: &'static str,
) -> LogScenario {
    without_line(
        snapshot_name,
        line,
        event(
            Level::WARN,
            "qdrant quadlet validation failed: missing provisioning dependency",
            &[("directive", directive), ("expected_dependency", expected_dependency)],
        ),
    )
}

pub(super) fn incorrect_requires_dependency() -> LogScenario {
    incorrect_dependency(
        "incorrect_requires_dependency",
        IncorrectDependencyCase {
            directive: "Requires",
            dependency: "network-online.target",
            expected_dependency: API_KEY_SERVICE,
            from: "Requires=repovec-qdrant-api-key.service repovec-provision.service",
            to: "Requires=network-online.target",
        },
    )
}

pub(super) fn incorrect_after_dependency() -> LogScenario {
    incorrect_dependency(
        "incorrect_after_dependency",
        IncorrectDependencyCase {
            directive: "After",
            dependency: "network-online.target",
            expected_dependency: API_KEY_SERVICE,
            from: "After=repovec-qdrant-api-key.service",
            to: "After=network-online.target",
        },
    )
}

fn incorrect_dependency(snapshot_name: &'static str, case: IncorrectDependencyCase) -> LogScenario {
    LogScenario {
        snapshot_name,
        contents: canonical().replace(case.from, case.to),
        expected: event(
            Level::WARN,
            "qdrant quadlet validation failed: incorrect provisioning dependency",
            &[
                ("directive", case.directive),
                ("dependency", case.dependency),
                ("expected_dependency", case.expected_dependency),
            ],
        ),
    }
}

pub(super) fn missing_api_key_secret() -> LogScenario {
    LogScenario {
        snapshot_name: "missing_api_key_secret",
        contents: canonical().replace(
            "Secret=repovec-qdrant-api-key,type=env,target=QDRANT__SERVICE__API_KEY\n",
            "",
        ),
        expected: event(
            Level::WARN,
            "qdrant quadlet validation failed: missing api key secret",
            &[
                ("expected_secret", "repovec-qdrant-api-key"),
                ("expected_target", "QDRANT__SERVICE__API_KEY"),
            ],
        ),
    }
}

pub(super) fn incorrect_api_key_secret() -> LogScenario {
    LogScenario {
        snapshot_name: "incorrect_api_key_secret",
        contents: canonical().replace("repovec-qdrant-api-key,type=env", "other-secret,type=env"),
        expected: event(
            Level::WARN,
            "qdrant quadlet validation failed: incorrect api key secret",
            &[
                ("secret", "other-secret,type=env,target=QDRANT__SERVICE__API_KEY"),
                ("expected_secret", "repovec-qdrant-api-key"),
                ("expected_target", "QDRANT__SERVICE__API_KEY"),
            ],
        ),
    }
}

pub(super) fn inline_api_key_environment() -> LogScenario {
    LogScenario {
        snapshot_name: "inline_api_key_environment",
        contents: canonical().replace(
            "Secret=repovec-qdrant-api-key,type=env,target=QDRANT__SERVICE__API_KEY\n",
            concat!(
                "Secret=repovec-qdrant-api-key,type=env,target=QDRANT__SERVICE__API_KEY\n",
                "Environment=QDRANT__SERVICE__API_KEY=secret\n",
            ),
        ),
        expected: event(
            Level::WARN,
            "qdrant quadlet validation failed: inline api key environment is disallowed",
            &[
                ("environment", "QDRANT__SERVICE__API_KEY=<redacted>"),
                ("expected_secret", "repovec-qdrant-api-key"),
                ("expected_target", "QDRANT__SERVICE__API_KEY"),
            ],
        ),
    }
}
