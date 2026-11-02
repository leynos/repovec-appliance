//! Tests for device-flow orchestration.

use std::{
    cell::RefCell,
    rc::Rc,
    time::{Duration, Instant},
};

use monotony::test_util::QueuedMonotonicClock;
use repovec_core::github_oauth::{AccessToken, ClientId, TokenPollOutcome};
use rstest::{fixture, rstest};

use super::{
    DeviceFlowLoginRequest,
    DeviceFlowRunError,
    DeviceFlowRuntime,
    TerminalDeviceFlowError,
    complete_device_flow,
};
use crate::tracing_test::capture_info_logs;

#[path = "tests_support.rs"]
mod support;

use support::{FakeApi, FakeApiError, FakeStore, FakeStoreError, RecordingSleeper};

#[rstest]
fn happy_path_stores_the_authorized_token(
    login_request: DeviceFlowLoginRequest,
    recording_sleeper: RecordingSleeper,
) {
    let api = FakeApi::new(vec![
        TokenPollOutcome::AuthorizationPending,
        TokenPollOutcome::Authorized(AccessToken::new("gho_secret", ["repo"])),
    ]);
    let store = FakeStore::new();

    let runtime = DeviceFlowRuntime::new(&api, &store, &recording_sleeper);
    let result = complete_device_flow(&runtime, &login_request, |_| {})
        .expect("device flow should complete");

    assert_eq!(result.prompt.user_code.as_str(), "ABCD-1234");
    assert_eq!(result.token.secret(), "gho_secret");
    assert_eq!(
        store.tokens.borrow().as_slice(),
        [AccessToken::new("gho_secret", ["repo"])]
    );
    assert_eq!(
        *recording_sleeper.sleeps.borrow(),
        [Duration::from_secs(5), Duration::from_secs(5)]
    );
}

#[rstest]
fn slow_down_increases_the_next_sleep(
    login_request: DeviceFlowLoginRequest,
    recording_sleeper: RecordingSleeper,
) {
    let api = FakeApi::new(vec![
        TokenPollOutcome::SlowDown,
        TokenPollOutcome::Authorized(AccessToken::new("gho_secret", ["repo"])),
    ]);
    let store = FakeStore::new();

    let runtime = DeviceFlowRuntime::new(&api, &store, &recording_sleeper);
    complete_device_flow(&runtime, &login_request, |_| {}).expect("device flow should complete");

    assert_eq!(
        *recording_sleeper.sleeps.borrow(),
        [Duration::from_secs(5), Duration::from_secs(10)]
    );
}

#[rstest]
fn access_denied_stops_without_storing_a_token(
    login_request: DeviceFlowLoginRequest,
    recording_sleeper: RecordingSleeper,
) {
    let api = FakeApi::new(vec![TokenPollOutcome::AccessDenied]);
    let store = FakeStore::new();

    let runtime = DeviceFlowRuntime::new(&api, &store, &recording_sleeper);
    let result = complete_device_flow(&runtime, &login_request, |_| {});

    assert!(matches!(
        result,
        Err(DeviceFlowRunError::Terminal(
            TerminalDeviceFlowError::AccessDenied
        )),
    ));
    assert!(store.tokens.borrow().is_empty());
}

#[rstest]
fn local_expiry_stops_polling_before_the_next_sleep_would_exceed_the_deadline(
    login_request: DeviceFlowLoginRequest,
    recording_sleeper: RecordingSleeper,
) {
    let api = FakeApi::new(vec![TokenPollOutcome::AuthorizationPending])
        .with_expires_in(Duration::from_secs(4));
    let store = FakeStore::new();

    let runtime = DeviceFlowRuntime::new(&api, &store, &recording_sleeper);
    let result = complete_device_flow(&runtime, &login_request, |_| {});

    assert!(matches!(
        result,
        Err(DeviceFlowRunError::Terminal(
            TerminalDeviceFlowError::ExpiredToken
        )),
    ));
    assert!(recording_sleeper.sleeps.borrow().is_empty());
}

#[rstest]
fn expiry_uses_the_injected_clock(login_request: DeviceFlowLoginRequest) {
    let api = FakeApi::new(vec![TokenPollOutcome::AuthorizationPending])
        .with_expires_in(Duration::from_secs(9));
    let store = FakeStore::new();
    let sleeper = RecordingSleeper {
        sleeps: RefCell::new(Vec::new()),
        events: None,
    };
    let started_at = Instant::now()
        .checked_add(Duration::from_secs(60))
        .expect("test instant should support adding one minute");
    let clock =
        QueuedMonotonicClock::from_instants([started_at, started_at + Duration::from_secs(5)]);

    let runtime = DeviceFlowRuntime::with_clock(&api, &store, &sleeper, &clock);
    let result = complete_device_flow(&runtime, &login_request, |_| {});

    assert!(matches!(
        result,
        Err(DeviceFlowRunError::Terminal(
            TerminalDeviceFlowError::ExpiredToken
        )),
    ));
    assert!(sleeper.sleeps.borrow().is_empty());
}

#[rstest]
fn local_expiry_emits_terminal_metric(login_request: DeviceFlowLoginRequest) {
    let api = FakeApi::new(vec![TokenPollOutcome::AuthorizationPending])
        .with_expires_in(Duration::from_secs(4));
    let store = FakeStore::new();
    let sleeper = RecordingSleeper {
        sleeps: RefCell::new(Vec::new()),
        events: None,
    };
    let runtime = DeviceFlowRuntime::new(&api, &store, &sleeper);

    let (result, logs) =
        capture_info_logs(|| complete_device_flow(&runtime, &login_request, |_| {}))
            .expect("capturing tracing logs should succeed");

    assert!(matches!(
        result,
        Err(DeviceFlowRunError::Terminal(
            TerminalDeviceFlowError::ExpiredToken
        )),
    ));
    assert!(logs.contains("metric.github_device_flow_terminal_total"));
    assert!(logs.contains("expired_token"));
}

#[rstest]
fn prompt_is_presented_before_first_sleep_or_poll(login_request: DeviceFlowLoginRequest) {
    let events = Rc::new(RefCell::new(Vec::new()));
    let api = FakeApi::new(vec![TokenPollOutcome::AccessDenied]).with_events(Rc::clone(&events));
    let store = FakeStore::new();
    let sleeper = RecordingSleeper {
        sleeps: RefCell::new(Vec::new()),
        events: Some(Rc::clone(&events)),
    };

    let runtime = DeviceFlowRuntime::new(&api, &store, &sleeper);
    let result = complete_device_flow(&runtime, &login_request, |prompt| {
        assert_eq!(prompt.user_code.as_str(), "ABCD-1234");
        events.borrow_mut().push("prompt");
    });

    assert!(matches!(
        result,
        Err(DeviceFlowRunError::Terminal(
            TerminalDeviceFlowError::AccessDenied
        )),
    ));
    assert_eq!(*events.borrow(), ["prompt", "sleep", "poll"]);
}

#[rstest]
fn reused_sleeper_starts_expiry_clock_per_login(
    login_request: DeviceFlowLoginRequest,
    recording_sleeper: RecordingSleeper,
) {
    let store = FakeStore::new();
    let first_api = FakeApi::new(vec![TokenPollOutcome::AccessDenied]);
    let first_runtime = DeviceFlowRuntime::new(&first_api, &store, &recording_sleeper);
    let first_result = complete_device_flow(&first_runtime, &login_request, |_| {});

    assert!(matches!(
        first_result,
        Err(DeviceFlowRunError::Terminal(
            TerminalDeviceFlowError::AccessDenied
        )),
    ));

    let second_api = FakeApi::new(vec![TokenPollOutcome::Authorized(AccessToken::new(
        "gho_secret",
        ["repo"],
    ))])
    .with_expires_in(Duration::from_secs(6));
    let second_runtime = DeviceFlowRuntime::new(&second_api, &store, &recording_sleeper);
    let second_result = complete_device_flow(&second_runtime, &login_request, |_| {});

    let completed = second_result.expect("second login should complete");
    assert_eq!(completed.token.secret(), "gho_secret");
    assert_eq!(
        store.tokens.borrow().as_slice(),
        [AccessToken::new("gho_secret", ["repo"])]
    );
}

#[rstest]
#[case::request_device_code(
    FakeApi::request_fails(FakeApiError::Request),
    FakeStore::new(),
    ExpectedRunError::OAuth(FakeApiError::Request)
)]
#[case::poll_token(
    FakeApi::poll_fails(FakeApiError::Poll),
    FakeStore::new(),
    ExpectedRunError::OAuth(FakeApiError::Poll)
)]
#[case::store(
    FakeApi::new(vec![TokenPollOutcome::Authorized(AccessToken::new("gho_secret", ["repo"]))]),
    FakeStore::failing(FakeStoreError::Store),
    ExpectedRunError::Storage(FakeStoreError::Store),
)]
fn adapter_errors_are_propagated(
    login_request: DeviceFlowLoginRequest,
    #[case] api: FakeApi,
    #[case] store: FakeStore,
    #[case] expected_error: ExpectedRunError,
) {
    let sleeper = RecordingSleeper {
        sleeps: RefCell::new(Vec::new()),
        events: None,
    };
    let runtime = DeviceFlowRuntime::new(&api, &store, &sleeper);
    let result = complete_device_flow(&runtime, &login_request, |_| {});

    match (result, expected_error) {
        (Err(DeviceFlowRunError::OAuth(actual)), ExpectedRunError::OAuth(expected_oauth)) => {
            assert_eq!(actual, expected_oauth);
        }
        (Err(DeviceFlowRunError::Storage(actual)), ExpectedRunError::Storage(expected_storage)) => {
            assert_eq!(actual, expected_storage);
        }
        (actual, unmatched_expected) => {
            panic!("expected {unmatched_expected:?}, got {actual:?}");
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ExpectedRunError {
    OAuth(FakeApiError),
    Storage(FakeStoreError),
}

#[fixture]
fn login_request() -> DeviceFlowLoginRequest {
    DeviceFlowLoginRequest {
        client_id: ClientId::new("client"),
        scopes: vec!["repo".into()],
    }
}

#[fixture]
fn recording_sleeper() -> RecordingSleeper {
    RecordingSleeper {
        sleeps: RefCell::new(Vec::new()),
        events: None,
    }
}
