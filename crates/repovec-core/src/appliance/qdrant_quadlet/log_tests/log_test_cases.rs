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

use super::{ExpectedEvent, LogScenario, canonical};

pub(super) fn invalid_line() -> LogScenario {
    LogScenario {
        contents: String::from("[Container]\nBearer very-secret-token\n"),
        expected: event(
            Level::ERROR,
            "qdrant quadlet validation rejected invalid line",
            &[("line_number", "2"), ("redacted_line", "Bearer <redacted>")],
        ),
    }
}

pub(super) fn property_before_section() -> LogScenario {
    LogScenario {
        contents: String::from("Environment=QDRANT__SERVICE__API_KEY=secret\n[Container]\n"),
        expected: event(
            Level::ERROR,
            "qdrant quadlet validation rejected property before section",
            &[("line_number", "1"), ("redacted_line", "Environment=<redacted>")],
        ),
    }
}

pub(super) fn missing_image() -> LogScenario {
    LogScenario {
        contents: canonical().replace("Image=docker.io/qdrant/qdrant:v1\n", ""),
        expected: event(
            Level::WARN,
            "qdrant quadlet validation failed: missing image",
            &[("expected_image", "docker.io/qdrant/qdrant:v1")],
        ),
    }
}

pub(super) fn image_not_pinned() -> LogScenario {
    LogScenario {
        contents: canonical().replace("docker.io/qdrant/qdrant:v1", "qdrant/qdrant:latest"),
        expected: event(
            Level::WARN,
            "qdrant quadlet validation failed: image is not fully qualified and pinned",
            &[("image", "qdrant/qdrant:latest"), ("expected_image", "docker.io/qdrant/qdrant:v1")],
        ),
    }
}

pub(super) fn unexpected_image() -> LogScenario {
    LogScenario {
        contents: canonical().replace("docker.io/qdrant/qdrant:v1", "docker.io/other/image:v2"),
        expected: event(
            Level::WARN,
            "qdrant quadlet validation failed: unexpected image",
            &[
                ("image", "docker.io/other/image:v2"),
                ("expected_image", "docker.io/qdrant/qdrant:v1"),
            ],
        ),
    }
}

pub(super) fn duplicate_image() -> LogScenario {
    LogScenario {
        contents: canonical().replace(
            "Image=docker.io/qdrant/qdrant:v1\n",
            "Image=docker.io/qdrant/qdrant:v1\nImage=docker.io/qdrant/qdrant:v2\n",
        ),
        expected: event(
            Level::WARN,
            "qdrant quadlet validation failed: unexpected image",
            &[("image", "docker.io/qdrant/qdrant:v1,docker.io/qdrant/qdrant:v2")],
        ),
    }
}

pub(super) fn missing_rest_publish() -> LogScenario {
    missing_publish("127.0.0.1:6333:6333", "6333")
}

pub(super) fn missing_grpc_publish() -> LogScenario {
    missing_publish("127.0.0.1:6334:6334", "6334")
}

fn missing_publish(port_mapping: &'static str, port: &'static str) -> LogScenario {
    LogScenario {
        contents: canonical().replace(&format!("PublishPort={port_mapping}\n"), ""),
        expected: event(
            Level::WARN,
            "qdrant quadlet validation failed: missing publish port",
            &[("port", port), ("expected_publish_port", port_mapping)],
        ),
    }
}

pub(super) fn rest_port_not_loopback() -> LogScenario {
    port_not_loopback("127.0.0.1:6333:6333", "0.0.0.0:6333:6333", "6333")
}

pub(super) fn grpc_port_not_loopback() -> LogScenario {
    port_not_loopback("127.0.0.1:6334:6334", "0.0.0.0:6334:6334", "6334")
}

fn port_not_loopback(
    from: &'static str,
    publish_port: &'static str,
    port: &'static str,
) -> LogScenario {
    LogScenario {
        contents: canonical().replace(from, publish_port),
        expected: event(
            Level::WARN,
            "qdrant quadlet validation failed: publish port is not bound to loopback",
            &[("port", port), ("publish_port", publish_port), ("expected_publish_port", from)],
        ),
    }
}

pub(super) fn missing_storage_mount() -> LogScenario {
    LogScenario {
        contents: canonical()
            .replace("Volume=/var/lib/repovec/qdrant-storage:/qdrant/storage:Z\n", ""),
        expected: event(
            Level::WARN,
            "qdrant quadlet validation failed: missing storage mount",
            &[
                ("expected_source", "/var/lib/repovec/qdrant-storage"),
                ("expected_target", "/qdrant/storage"),
            ],
        ),
    }
}

pub(super) fn incorrect_storage_source() -> LogScenario {
    LogScenario {
        contents: canonical()
            .replace("/var/lib/repovec/qdrant-storage", "/var/lib/other/qdrant-storage"),
        expected: event(
            Level::WARN,
            "qdrant quadlet validation failed: incorrect storage source",
            &[
                ("source", "/var/lib/other/qdrant-storage"),
                ("expected_source", "/var/lib/repovec/qdrant-storage"),
            ],
        ),
    }
}

pub(super) fn incorrect_storage_target() -> LogScenario {
    LogScenario {
        contents: canonical().replace("/qdrant/storage:Z", "/srv/qdrant:Z"),
        expected: event(
            Level::WARN,
            "qdrant quadlet validation failed: incorrect storage target",
            &[("storage_target", "/srv/qdrant"), ("expected_target", "/qdrant/storage")],
        ),
    }
}

pub(super) fn missing_selinux_relabel() -> LogScenario {
    LogScenario {
        contents: canonical().replace(":/qdrant/storage:Z", ":/qdrant/storage"),
        expected: event(
            Level::WARN,
            "qdrant quadlet validation failed: missing selinux relabel",
            &[("volume", "/var/lib/repovec/qdrant-storage:/qdrant/storage")],
        ),
    }
}

pub(super) fn missing_auto_update() -> LogScenario {
    LogScenario {
        contents: canonical().replace("AutoUpdate=registry\n", ""),
        expected: event(
            Level::WARN,
            "qdrant quadlet validation failed: missing auto-update policy",
            &[("expected_auto_update", "registry")],
        ),
    }
}

pub(super) fn incorrect_auto_update() -> LogScenario {
    LogScenario {
        contents: canonical().replace("AutoUpdate=registry\n", "AutoUpdate=local\n"),
        expected: event(
            Level::WARN,
            "qdrant quadlet validation failed: incorrect auto-update policy",
            &[("auto_update", "local"), ("expected_auto_update", "registry")],
        ),
    }
}

pub(super) fn missing_requires_dependency() -> LogScenario {
    missing_dependency("Requires=repovec-qdrant-api-key.service\n", "Requires")
}

pub(super) fn missing_after_dependency() -> LogScenario {
    missing_dependency("After=repovec-qdrant-api-key.service\n", "After")
}

fn missing_dependency(line: &str, directive: &'static str) -> LogScenario {
    LogScenario {
        contents: canonical().replace(line, ""),
        expected: event(
            Level::WARN,
            "qdrant quadlet validation failed: missing api key provisioning dependency",
            &[("directive", directive), ("expected_dependency", "repovec-qdrant-api-key.service")],
        ),
    }
}

pub(super) fn incorrect_requires_dependency() -> LogScenario {
    incorrect_dependency(
        "Requires=repovec-qdrant-api-key.service",
        "Requires=network-online.target",
        "Requires",
    )
}

pub(super) fn incorrect_after_dependency() -> LogScenario {
    incorrect_dependency(
        "After=repovec-qdrant-api-key.service",
        "After=network-online.target",
        "After",
    )
}

fn incorrect_dependency(from: &str, to: &str, directive: &'static str) -> LogScenario {
    LogScenario {
        contents: canonical().replace(from, to),
        expected: event(
            Level::WARN,
            "qdrant quadlet validation failed: incorrect api key provisioning dependency",
            &[
                ("directive", directive),
                ("dependency", "network-online.target"),
                ("expected_dependency", "repovec-qdrant-api-key.service"),
            ],
        ),
    }
}

pub(super) fn missing_api_key_secret() -> LogScenario {
    LogScenario {
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
        contents: canonical().replace(
            "Secret=repovec-qdrant-api-key,type=env,target=QDRANT__SERVICE__API_KEY\n",
            "Secret=repovec-qdrant-api-key,type=env,target=QDRANT__SERVICE__API_KEY\nEnvironment=QDRANT__SERVICE__API_KEY=secret\n",
        ),
        expected: event(
            Level::WARN,
            "qdrant quadlet validation failed: inline api key environment is disallowed",
            &[("environment", "QDRANT__SERVICE__API_KEY=<redacted>"), ("expected_secret", "repovec-qdrant-api-key"), ("expected_target", "QDRANT__SERVICE__API_KEY")],
        ),
    }
}

fn event(
    level: Level,
    message: &'static str,
    fields: &[(&'static str, &'static str)],
) -> ExpectedEvent {
    ExpectedEvent { level, message, fields: fields.to_vec() }
}
