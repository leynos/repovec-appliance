//! Mutation cases used by Qdrant Quadlet log-contract tests.
//!
//! This child module keeps telemetry fixtures separate from the subscriber
//! harness in `mod.rs`, so each validation failure maps to one compact
//! `LogScenario`. The cases mutate the canonical checked-in Quadlet to exercise
//! parser, structural, and API-key contract boundaries exposed by the parent
//! `qdrant_quadlet` module. Keeping the expected fields beside each mutation
//! makes log-contract review local and prevents unrelated test infrastructure
//! from obscuring operator-facing event changes.

use tracing::Level;

use super::{LogScenario, canonical, log_test_case_support::event};

/// Declares one scenario constructor per mutation.
///
/// Every constructor shares the same shape: name the scenario, edit the
/// canonical Quadlet, and pair that with the event the observer must emit. The
/// snapshot name is the constructor name, so the table below records only the
/// data that actually differs between cases.
macro_rules! log_scenarios {
    ($($name:ident, $contents:expr, $level:expr, $message:literal, $fields:expr;)*) => {
        $(
            pub(super) fn $name() -> LogScenario {
                LogScenario {
                    snapshot_name: stringify!($name),
                    contents: $contents,
                    expected: event($level, $message, $fields),
                }
            }
        )*
    };
}

log_scenarios! {
    invalid_line,
    String::from("[Container]\nBearer very-secret-token\n"),
    Level::ERROR,
    "qdrant quadlet validation rejected invalid line",
    &[("line_number", "2"), ("redacted_line", "Bearer <redacted>")];
    property_before_section,
    String::from("Environment=QDRANT__SERVICE__API_KEY=secret\n[Container]\n"),
    Level::ERROR,
    "qdrant quadlet validation rejected property before section",
    &[
        ("line_number", "1"),
        ("redacted_line", "Environment=<redacted>"),
    ];
    bearer_assignment_before_section,
    String::from("auth=Bearer very-secret-token\n[Container]\n"),
    Level::ERROR,
    "qdrant quadlet validation rejected property before section",
    &[("line_number", "1"), ("redacted_line", "Bearer <redacted>")];
    missing_image,
    canonical().replace("Image=docker.io/qdrant/qdrant:v1\n", ""),
    Level::WARN,
    "qdrant quadlet validation failed: missing image",
    &[("expected_image", "docker.io/qdrant/qdrant:v1")];
    image_not_pinned,
    canonical().replace("docker.io/qdrant/qdrant:v1", "qdrant/qdrant:latest"),
    Level::WARN,
    "qdrant quadlet validation failed: image is not fully qualified and pinned",
    &[
        ("image", "qdrant/qdrant:latest"),
        ("expected_image", "docker.io/qdrant/qdrant:v1"),
    ];
    unexpected_image,
    canonical().replace("docker.io/qdrant/qdrant:v1", "docker.io/other/image:v2"),
    Level::WARN,
    "qdrant quadlet validation failed: unexpected image",
    &[
        ("image", "docker.io/other/image:v2"),
        ("expected_image", "docker.io/qdrant/qdrant:v1"),
    ];
    duplicate_image,
    canonical().replace(
        "Image=docker.io/qdrant/qdrant:v1\n",
        "Image=docker.io/qdrant/qdrant:v1\nImage=docker.io/qdrant/qdrant:v2\n",
    ),
    Level::WARN,
    "qdrant quadlet validation failed: unexpected image",
    &[(
        "image",
        "docker.io/qdrant/qdrant:v1,docker.io/qdrant/qdrant:v2",
    )];
    missing_storage_mount,
    canonical().replace(
        "Volume=/var/lib/repovec/qdrant-storage:/qdrant/storage:Z\n",
        "",
    ),
    Level::WARN,
    "qdrant quadlet validation failed: missing storage mount",
    &[
        ("expected_source", "/var/lib/repovec/qdrant-storage"),
        ("expected_target", "/qdrant/storage"),
    ];
    malformed_storage_mount,
    canonical().replace(
        "Volume=/var/lib/repovec/qdrant-storage:/qdrant/storage:Z",
        "Volume=/var/lib/repovec/qdrant-storage",
    ),
    Level::WARN,
    "qdrant quadlet validation failed: missing storage mount",
    &[
        ("volume", "/var/lib/repovec/qdrant-storage"),
        ("expected_source", "/var/lib/repovec/qdrant-storage"),
        ("expected_target", "/qdrant/storage"),
    ];
    incorrect_storage_source,
    canonical().replace(
        "/var/lib/repovec/qdrant-storage",
        "/var/lib/other/qdrant-storage",
    ),
    Level::WARN,
    "qdrant quadlet validation failed: incorrect storage source",
    &[
        ("source", "/var/lib/other/qdrant-storage"),
        ("expected_source", "/var/lib/repovec/qdrant-storage"),
    ];
    incorrect_storage_target,
    canonical().replace("/qdrant/storage:Z", "/srv/qdrant:Z"),
    Level::WARN,
    "qdrant quadlet validation failed: incorrect storage target",
    &[
        ("storage_target", "/srv/qdrant"),
        ("expected_target", "/qdrant/storage"),
    ];
    missing_selinux_relabel,
    canonical().replace(":/qdrant/storage:Z", ":/qdrant/storage"),
    Level::WARN,
    "qdrant quadlet validation failed: missing selinux relabel",
    &[("volume", "/var/lib/repovec/qdrant-storage:/qdrant/storage")];
    missing_auto_update,
    canonical().replace("AutoUpdate=registry\n", ""),
    Level::WARN,
    "qdrant quadlet validation failed: missing auto-update policy",
    &[("expected_auto_update", "registry")];
    incorrect_auto_update,
    canonical().replace("AutoUpdate=registry\n", "AutoUpdate=local\n"),
    Level::WARN,
    "qdrant quadlet validation failed: incorrect auto-update policy",
    &[
        ("auto_update", "local"),
        ("expected_auto_update", "registry"),
    ];
}

pub(super) fn missing_rest_publish() -> LogScenario {
    missing_publish("missing_rest_publish", "127.0.0.1:6333:6333", "6333")
}

pub(super) fn missing_grpc_publish() -> LogScenario {
    missing_publish("missing_grpc_publish", "127.0.0.1:6334:6334", "6334")
}

fn missing_publish(
    snapshot_name: &'static str,
    port_mapping: &'static str,
    port: &'static str,
) -> LogScenario {
    LogScenario {
        snapshot_name,
        contents: canonical().replace(&format!("PublishPort={port_mapping}\n"), ""),
        expected: event(
            Level::WARN,
            "qdrant quadlet validation failed: missing publish port",
            &[("port", port), ("expected_publish_port", port_mapping)],
        ),
    }
}

pub(super) fn rest_port_not_loopback() -> LogScenario {
    port_not_loopback(
        "rest_port_not_loopback",
        "127.0.0.1:6333:6333",
        "0.0.0.0:6333:6333",
        "6333",
    )
}

pub(super) fn grpc_port_not_loopback() -> LogScenario {
    port_not_loopback(
        "grpc_port_not_loopback",
        "127.0.0.1:6334:6334",
        "0.0.0.0:6334:6334",
        "6334",
    )
}

fn port_not_loopback(
    snapshot_name: &'static str,
    from: &'static str,
    publish_port: &'static str,
    port: &'static str,
) -> LogScenario {
    LogScenario {
        snapshot_name,
        contents: canonical().replace(from, publish_port),
        expected: event(
            Level::WARN,
            "qdrant quadlet validation failed: publish port is not bound to loopback",
            &[
                ("port", port),
                ("publish_port", publish_port),
                ("expected_publish_port", from),
            ],
        ),
    }
}
