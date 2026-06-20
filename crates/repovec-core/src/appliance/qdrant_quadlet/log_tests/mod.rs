//! Log-contract tests for Qdrant Quadlet validation telemetry.
//!
//! This module owns the tracing subscriber test harness used to assert the
//! public observer adapter emits stable structured events. It exercises the
//! parent `qdrant_quadlet` validation entry points end to end, while the
//! sibling `log_test_cases` module provides the malformed Quadlet fixtures and
//! expected event contracts. These tests complement the behaviour snapshots in
//! `tests.rs` by failing when operator-facing telemetry is removed or renamed.

mod log_test_cases;

use std::{
    collections::BTreeMap,
    fmt,
    sync::{Arc, Mutex},
};

use insta::assert_debug_snapshot;
use log_test_cases::{
    bearer_assignment_before_section, duplicate_image, grpc_port_not_loopback, image_not_pinned,
    incorrect_after_dependency, incorrect_api_key_secret, incorrect_auto_update,
    incorrect_requires_dependency, incorrect_storage_source, incorrect_storage_target,
    inline_api_key_environment, invalid_line, malformed_storage_mount, missing_after_dependency,
    missing_api_key_secret, missing_auto_update, missing_grpc_publish, missing_image,
    missing_requires_dependency, missing_rest_publish, missing_selinux_relabel,
    missing_storage_mount, property_before_section, rest_port_not_loopback, unexpected_image,
};
use rstest::rstest;
use tracing::{
    Event, Level, Subscriber,
    field::{Field, Visit},
};
use tracing_subscriber::{Layer, layer::Context, prelude::*};

use super::{
    CHECKED_IN_QDRANT_QUADLET_PATH, LOG_TARGET, TracingQdrantQuadletObserver,
    checked_in_qdrant_quadlet, validate_checked_in_qdrant_quadlet, validate_qdrant_quadlet,
};

#[derive(Clone, Debug)]
struct CapturedEvent {
    level: Level,
    target: String,
    fields: BTreeMap<String, String>,
}

#[derive(Clone, Debug, Default)]
struct CapturedEvents {
    events: Arc<Mutex<Vec<CapturedEvent>>>,
}

impl CapturedEvents {
    fn collect_from(validation: impl FnOnce()) -> Vec<CapturedEvent> {
        let collector = Self::default();
        let subscriber = tracing_subscriber::registry().with(collector.clone());
        tracing::subscriber::with_default(subscriber, validation);
        match collector.events.lock() {
            Ok(events) => events.clone(),
            Err(poisoned) => panic!("failed to lock collector.events: {poisoned}"),
        }
    }
}

impl<S> Layer<S> for CapturedEvents
where
    S: Subscriber,
{
    fn on_event(&self, event: &Event<'_>, _ctx: Context<'_, S>) {
        let mut visitor = EventVisitor::default();
        event.record(&mut visitor);
        let mut events = match self.events.lock() {
            Ok(events) => events,
            Err(error) => panic!("failed to lock captured qdrant quadlet events: {error}"),
        };
        events.push(CapturedEvent {
            level: *event.metadata().level(),
            target: event.metadata().target().to_owned(),
            fields: visitor.fields,
        });
    }
}

#[derive(Default)]
struct EventVisitor {
    fields: BTreeMap<String, String>,
}

impl Visit for EventVisitor {
    fn record_str(&mut self, field: &Field, value: &str) {
        self.fields.insert(field.name().to_owned(), value.to_owned());
    }

    fn record_i64(&mut self, field: &Field, value: i64) {
        self.fields.insert(field.name().to_owned(), value.to_string());
    }

    fn record_u64(&mut self, field: &Field, value: u64) {
        self.fields.insert(field.name().to_owned(), value.to_string());
    }

    fn record_debug(&mut self, field: &Field, value: &dyn fmt::Debug) {
        self.fields.insert(field.name().to_owned(), format!("{value:?}"));
    }
}

struct ExpectedEvent {
    level: Level,
    message: &'static str,
    fields: Vec<(&'static str, &'static str)>,
}

struct LogScenario {
    snapshot_name: &'static str,
    contents: String,
    expected: ExpectedEvent,
}

#[test]
fn checked_in_qdrant_quadlet_validation_emits_entry_and_success_events() {
    let events = CapturedEvents::collect_from(|| {
        validate_checked_in_qdrant_quadlet(&TracingQdrantQuadletObserver)
            .expect("checked-in Qdrant Quadlet should remain valid");
    });

    assert_event(
        &events,
        ExpectedEvent {
            level: Level::INFO,
            message: "validating checked-in qdrant quadlet",
            fields: vec![("path", CHECKED_IN_QDRANT_QUADLET_PATH)],
        },
    );
    assert_event(
        &events,
        ExpectedEvent {
            level: Level::INFO,
            message: "validating qdrant quadlet contract",
            fields: Vec::new(),
        },
    );
    assert_event(
        &events,
        ExpectedEvent {
            level: Level::INFO,
            message: "qdrant quadlet contract validation succeeded",
            fields: Vec::new(),
        },
    );
}

#[test]
fn inline_qdrant_quadlet_validation_emits_entry_and_success_events() {
    let events = CapturedEvents::collect_from(|| {
        validate_qdrant_quadlet(checked_in_qdrant_quadlet(), &TracingQdrantQuadletObserver)
            .expect("checked-in Qdrant Quadlet should remain valid");
    });

    assert_event(
        &events,
        ExpectedEvent {
            level: Level::INFO,
            message: "validating qdrant quadlet contract",
            fields: Vec::new(),
        },
    );
    assert_event(
        &events,
        ExpectedEvent {
            level: Level::INFO,
            message: "qdrant quadlet contract validation succeeded",
            fields: Vec::new(),
        },
    );
}

#[test]
fn parser_redacts_nested_environment_assignment_event_shape() {
    let scenario = property_before_section();
    let events = CapturedEvents::collect_from(|| {
        validate_qdrant_quadlet(&scenario.contents, &TracingQdrantQuadletObserver)
            .expect_err("scenario should violate the Qdrant Quadlet contract");
    });

    assert_event(&events, scenario.expected);
    assert_debug_snapshot!("parser_redacts_nested_environment_assignment_event_shape", events);
}

#[rstest]
#[case::invalid_line(invalid_line())]
#[case::property_before_section(property_before_section())]
#[case::bearer_assignment_before_section(bearer_assignment_before_section())]
#[case::missing_image(missing_image())]
#[case::image_not_pinned(image_not_pinned())]
#[case::unexpected_image(unexpected_image())]
#[case::duplicate_image(duplicate_image())]
#[case::missing_rest_publish(missing_rest_publish())]
#[case::missing_grpc_publish(missing_grpc_publish())]
#[case::rest_port_not_loopback(rest_port_not_loopback())]
#[case::grpc_port_not_loopback(grpc_port_not_loopback())]
#[case::missing_storage_mount(missing_storage_mount())]
#[case::malformed_storage_mount(malformed_storage_mount())]
#[case::incorrect_storage_source(incorrect_storage_source())]
#[case::incorrect_storage_target(incorrect_storage_target())]
#[case::missing_selinux_relabel(missing_selinux_relabel())]
#[case::missing_auto_update(missing_auto_update())]
#[case::incorrect_auto_update(incorrect_auto_update())]
#[case::missing_requires_dependency(missing_requires_dependency())]
#[case::missing_after_dependency(missing_after_dependency())]
#[case::incorrect_requires_dependency(incorrect_requires_dependency())]
#[case::incorrect_after_dependency(incorrect_after_dependency())]
#[case::missing_api_key_secret(missing_api_key_secret())]
#[case::incorrect_api_key_secret(incorrect_api_key_secret())]
#[case::inline_api_key_environment(inline_api_key_environment())]
fn qdrant_quadlet_validation_emits_contract_violation_events(#[case] scenario: LogScenario) {
    let events = CapturedEvents::collect_from(|| {
        validate_qdrant_quadlet(&scenario.contents, &TracingQdrantQuadletObserver)
            .expect_err("scenario should violate the Qdrant Quadlet contract");
    });

    assert_event(&events, scenario.expected);
    assert_debug_snapshot!(scenario.snapshot_name, events);
}

fn assert_event(events: &[CapturedEvent], expected: ExpectedEvent) {
    let Some(event) = events.iter().find(|event| field_matches(event, "message", expected.message))
    else {
        panic!("missing event `{}` in {events:#?}", expected.message);
    };

    assert_eq!(event.level, expected.level);
    assert_eq!(event.target, LOG_TARGET);
    for (field, value) in expected.fields {
        assert!(field_matches(event, field, value), "missing field {field}={value} in {event:#?}");
    }
}

fn field_matches(event: &CapturedEvent, field: &str, expected: &str) -> bool {
    event
        .fields
        .get(field)
        .is_some_and(|actual| actual == expected || actual == &format!("{expected:?}"))
}

fn canonical() -> String { checked_in_qdrant_quadlet().to_owned() }
