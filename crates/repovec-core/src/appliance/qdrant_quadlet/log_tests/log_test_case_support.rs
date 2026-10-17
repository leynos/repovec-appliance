//! Shared fixtures for Qdrant Quadlet log-contract test cases.
//!
//! Both mutation families (`log_test_cases` for structural and API-key contract
//! failures, `log_test_dependency_cases` for provisioning-dependency failures)
//! build their scenarios from the same canonical Quadlet and emit their
//! expectations as `ExpectedEvent` values. The shared constructors live here so
//! each case module only carries its own mutations.

use tracing::Level;

use super::{ExpectedEvent, LogScenario, canonical};

/// Builds the expected structured event for a scenario.
pub(super) fn event(
    level: Level,
    message: &'static str,
    fields: &[(&'static str, &'static str)],
) -> ExpectedEvent {
    ExpectedEvent { level, message, fields: fields.to_vec() }
}

/// Builds a scenario whose Quadlet is the canonical asset with `line` removed.
pub(super) fn without_line(
    snapshot_name: &'static str,
    line: &str,
    expected: ExpectedEvent,
) -> LogScenario {
    LogScenario { snapshot_name, contents: canonical().replace(line, ""), expected }
}
