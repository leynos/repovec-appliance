//! Live-layout startup cases for shared daemon startup checks.
//!
//! These cover the fail-closed boundary between the static systemd contract,
//! the live directory pre-flight, and the Qdrant probe, plus the observability
//! the live failure leaves behind.

use std::cell::Cell;

use super::{
    DaemonStartupError, STARTUP_FAILURE_EXIT_CODE, log_daemon_startup_error,
    validate_daemon_startup_contracts_with_live,
};
use crate::appliance::directory_layout::live::{LiveLayoutError, LiveLayoutViolation};

/// Build the live-layout failure every live-boundary case injects.
///
/// A missing path is the operator-facing case: provisioning did not run, or ran
/// against a different root. The path is carried through to the log line, so
/// each assertion can name it rather than matching on text alone.
fn missing_data_root() -> LiveLayoutError {
    LiveLayoutError::MissingPath { path: "/var/lib/repovec".into() }
}

/// Build an ownership violation with distinct expected and observed ids.
fn wrong_owner() -> LiveLayoutError {
    LiveLayoutError::IncorrectOwner {
        violation: LiveLayoutViolation {
            path: "/var/lib/repovec/qdrant-storage".into(),
            expected: "root (uid 0)".to_owned(),
            actual: "uid 998".to_owned(),
        },
    }
}

/// A live-layout violation must abort startup with the typed error, not fall
/// through.
#[test]
fn startup_reports_live_layout_violation() {
    let result = validate_daemon_startup_contracts_with_live(
        || Ok(()),
        || Err(missing_data_root()),
        || async { Ok(()) },
    );

    let Err(DaemonStartupError::LiveLayout(error)) = result else {
        panic!("a live layout violation must surface as DaemonStartupError::LiveLayout");
    };
    assert_eq!(error.path(), Some("/var/lib/repovec"));
}

/// A failed live pre-flight must not reach the Qdrant probe: the data tree is
/// supposed to hold the API key, so probing would fail for the wrong reason.
#[test]
fn startup_skips_qdrant_after_live_layout_failure() {
    let qdrant_called = Cell::new(false);

    let result = validate_daemon_startup_contracts_with_live(
        || Ok(()),
        || Err(missing_data_root()),
        || {
            qdrant_called.set(true);
            async { Ok(()) }
        },
    );

    assert!(matches!(result, Err(DaemonStartupError::LiveLayout(_))));
    assert!(!qdrant_called.get(), "the Qdrant probe must be suppressed");
}

/// The live pre-flight runs after the static systemd contract and before the
/// Qdrant probe, so an operator sees the cheapest failure first.
#[test]
fn startup_runs_live_layout_check_between_the_other_two() {
    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    enum Step {
        Systemd,
        LiveLayout,
        Qdrant,
    }

    let observed = std::cell::RefCell::new(Vec::new());

    let result = validate_daemon_startup_contracts_with_live(
        || {
            observed.borrow_mut().push(Step::Systemd);
            Ok(())
        },
        || {
            observed.borrow_mut().push(Step::LiveLayout);
            Ok(())
        },
        || {
            observed.borrow_mut().push(Step::Qdrant);
            async { Ok(()) }
        },
    );

    assert!(result.is_ok());
    assert_eq!(observed.into_inner(), vec![Step::Systemd, Step::LiveLayout, Step::Qdrant]);
}

/// Retain the offending path in the operator log and map the failure to exit 1.
#[test]
fn live_layout_startup_failure_names_the_path_and_exits_1() -> Result<(), String> {
    let (exit_code_result, logs) = repovec_test_helpers::capture_logs(|| {
        validate_daemon_startup_contracts_with_live(
            || Ok(()),
            || Err(missing_data_root()),
            || async { Ok(()) },
        )
        .err()
        .map(log_daemon_startup_error)
        .ok_or_else(|| "a live layout violation must abort the startup boundary".to_owned())
    })?;
    let exit_code = exit_code_result?;

    repovec_test_helpers::ensure(
        exit_code == STARTUP_FAILURE_EXIT_CODE,
        "a live layout failure must map to the startup failure exit code",
    )?;
    repovec_test_helpers::ensure_log_line_contains(
        &logs,
        "ERROR",
        "path=/var/lib/repovec",
        "the live layout failure log must name the offending path",
    )?;
    repovec_test_helpers::ensure_log_line_contains(
        &logs,
        "ERROR",
        "live directory layout violation - aborting startup",
        "the live layout failure log must state that startup is aborting",
    )
}

/// The failure log carries the stable category so a dashboard can group
/// failures without parsing free-form text or indexing on a host path.
#[test]
fn live_layout_failure_logs_a_bounded_category() -> Result<(), String> {
    let (exit_code, logs) = repovec_test_helpers::capture_logs(|| {
        log_daemon_startup_error(DaemonStartupError::LiveLayout(missing_data_root()))
    })?;

    repovec_test_helpers::ensure(
        exit_code == STARTUP_FAILURE_EXIT_CODE,
        "a live layout failure must map to the startup failure exit code",
    )?;
    repovec_test_helpers::ensure_log_line_contains(
        &logs,
        "ERROR",
        r#"error_category="missing_path""#,
        "the failure log must carry the bounded category",
    )
}

/// Ownership violations log both sides of the comparison, matching on disk
/// against the contract, so an operator can tell a wrong uid from a wrong
/// directory.
#[test]
fn live_layout_failure_logs_expected_and_actual_identity() -> Result<(), String> {
    let (_, logs) = repovec_test_helpers::capture_logs(|| {
        log_daemon_startup_error(DaemonStartupError::LiveLayout(wrong_owner()))
    })?;

    repovec_test_helpers::ensure_log_line_contains(
        &logs,
        "ERROR",
        r#"expected="root (uid 0)""#,
        "the failure log must name the expected owner",
    )?;
    repovec_test_helpers::ensure_log_line_contains(
        &logs,
        "ERROR",
        r#"actual="uid 998""#,
        "the failure log must name the observed owner",
    )?;
    repovec_test_helpers::ensure_log_line_contains(
        &logs,
        "ERROR",
        r#"error_category="incorrect_owner""#,
        "the failure log must carry the owner category",
    )
}

/// The metric event carries only the category, never a path, so its label
/// space stays bounded.
#[test]
fn live_layout_failure_emits_a_category_only_metric() -> Result<(), String> {
    let (_, logs) = repovec_test_helpers::capture_logs(|| {
        log_daemon_startup_error(DaemonStartupError::LiveLayout(wrong_owner()))
    })?;

    repovec_test_helpers::ensure_log_line_contains(
        &logs,
        "INFO",
        "metric.daemon_startup_live_layout_failure_total",
        "the metric event must be emitted",
    )?;
    repovec_test_helpers::ensure_log_line_contains(
        &logs,
        "INFO",
        r#"error_category="incorrect_owner""#,
        "the metric event must carry the category",
    )
}
