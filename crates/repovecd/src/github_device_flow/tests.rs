//! Tests for device-flow orchestration.

use std::{cell::RefCell, rc::Rc, time::Duration};

use repovec_core::github_oauth::{AccessToken, ClientId, DeviceCode, TokenPollOutcome, UserCode};
use rstest::{fixture, rstest};
use thiserror::Error;

use super::{
    DeviceAuthorization, DeviceFlowApi, DeviceFlowLoginRequest, DeviceFlowRunError,
    DeviceFlowRuntime, Sleeper, TerminalDeviceFlowError, TokenStore, complete_device_flow,
};

struct FakeApi {
    outcomes: RefCell<Vec<TokenPollOutcome>>,
    expires_in: Duration,
    events: Option<Rc<RefCell<Vec<&'static str>>>>,
    request_error: Option<FakeApiError>,
    poll_error: Option<FakeApiError>,
}

impl FakeApi {
    fn new(outcomes: Vec<TokenPollOutcome>) -> Self {
        Self {
            outcomes: RefCell::new(outcomes),
            expires_in: Duration::from_secs(900),
            events: None,
            request_error: None,
            poll_error: None,
        }
    }

    fn with_expires_in(mut self, expires_in: Duration) -> Self {
        self.expires_in = expires_in;
        self
    }

    fn with_events(mut self, events: Rc<RefCell<Vec<&'static str>>>) -> Self {
        self.events = Some(events);
        self
    }

    fn request_fails(error: FakeApiError) -> Self {
        Self { request_error: Some(error), ..Self::new(Vec::new()) }
    }

    fn poll_fails(error: FakeApiError) -> Self {
        Self { poll_error: Some(error), ..Self::new(Vec::new()) }
    }
}

impl DeviceFlowApi for FakeApi {
    type Error = FakeApiError;

    fn request_device_code(
        &self,
        _client_id: &ClientId,
        _scopes: &[String],
    ) -> Result<DeviceAuthorization, Self::Error> {
        if let Some(error) = self.request_error {
            return Err(error);
        }
        Ok(DeviceAuthorization {
            device_code: DeviceCode::new("device"),
            user_code: UserCode::new("ABCD-1234"),
            verification_uri: "https://github.com/login/device".to_owned(),
            expires_in: self.expires_in,
            interval: Duration::from_secs(5),
        })
    }

    fn poll_token(
        &self,
        _client_id: &ClientId,
        _authorization: &DeviceAuthorization,
    ) -> Result<TokenPollOutcome, Self::Error> {
        if let Some(events) = &self.events {
            events.borrow_mut().push("poll");
        }
        if let Some(error) = self.poll_error {
            return Err(error);
        }
        Ok(self.outcomes.borrow_mut().remove(0))
    }
}

#[derive(Clone, Copy, Debug, Error, Eq, PartialEq)]
enum FakeApiError {
    #[error("request failed")]
    Request,
    #[error("poll failed")]
    Poll,
}

struct FakeStore {
    tokens: RefCell<Vec<AccessToken>>,
    error: Option<FakeStoreError>,
}

impl FakeStore {
    fn new() -> Self { Self { tokens: RefCell::new(Vec::new()), error: None } }

    fn failing(error: FakeStoreError) -> Self {
        Self { tokens: RefCell::new(Vec::new()), error: Some(error) }
    }
}

impl TokenStore for FakeStore {
    type Error = FakeStoreError;

    fn store(&self, token: &AccessToken) -> Result<(), Self::Error> {
        if let Some(error) = self.error {
            return Err(error);
        }
        self.tokens.borrow_mut().push(token.clone());
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Error, Eq, PartialEq)]
enum FakeStoreError {
    #[error("store failed")]
    Store,
}

struct RecordingSleeper {
    sleeps: RefCell<Vec<Duration>>,
    events: Option<Rc<RefCell<Vec<&'static str>>>>,
}

impl Sleeper for RecordingSleeper {
    fn sleep(&self, duration: Duration) {
        if let Some(events) = &self.events {
            events.borrow_mut().push("sleep");
        }
        self.sleeps.borrow_mut().push(duration);
    }
}

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
    assert_eq!(store.tokens.borrow().as_slice(), [AccessToken::new("gho_secret", ["repo"])]);
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
        Err(DeviceFlowRunError::Terminal(TerminalDeviceFlowError::AccessDenied)),
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
        Err(DeviceFlowRunError::Terminal(TerminalDeviceFlowError::ExpiredToken)),
    ));
    assert!(recording_sleeper.sleeps.borrow().is_empty());
}

#[rstest]
fn prompt_is_presented_before_first_sleep_or_poll(login_request: DeviceFlowLoginRequest) {
    let events = Rc::new(RefCell::new(Vec::new()));
    let api = FakeApi::new(vec![TokenPollOutcome::AccessDenied]).with_events(Rc::clone(&events));
    let store = FakeStore::new();
    let sleeper =
        RecordingSleeper { sleeps: RefCell::new(Vec::new()), events: Some(Rc::clone(&events)) };

    let runtime = DeviceFlowRuntime::new(&api, &store, &sleeper);
    let result = complete_device_flow(&runtime, &login_request, |prompt| {
        assert_eq!(prompt.user_code.as_str(), "ABCD-1234");
        events.borrow_mut().push("prompt");
    });

    assert!(matches!(
        result,
        Err(DeviceFlowRunError::Terminal(TerminalDeviceFlowError::AccessDenied)),
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
        Err(DeviceFlowRunError::Terminal(TerminalDeviceFlowError::AccessDenied)),
    ));

    let second_api =
        FakeApi::new(vec![TokenPollOutcome::Authorized(AccessToken::new("gho_secret", ["repo"]))])
            .with_expires_in(Duration::from_secs(6));
    let second_runtime = DeviceFlowRuntime::new(&second_api, &store, &recording_sleeper);
    let second_result = complete_device_flow(&second_runtime, &login_request, |_| {});

    let completed = second_result.expect("second login should complete");
    assert_eq!(completed.token.secret(), "gho_secret");
    assert_eq!(store.tokens.borrow().as_slice(), [AccessToken::new("gho_secret", ["repo"])]);
}

#[rstest]
fn request_device_code_errors_are_propagated(login_request: DeviceFlowLoginRequest) {
    let api = FakeApi::request_fails(FakeApiError::Request);
    let store = FakeStore::new();
    let sleeper = RecordingSleeper { sleeps: RefCell::new(Vec::new()), events: None };

    let runtime = DeviceFlowRuntime::new(&api, &store, &sleeper);
    let result = complete_device_flow(&runtime, &login_request, |_| {});

    assert!(matches!(result, Err(DeviceFlowRunError::OAuth(FakeApiError::Request))));
}

#[rstest]
fn poll_token_errors_are_propagated(login_request: DeviceFlowLoginRequest) {
    let api = FakeApi::poll_fails(FakeApiError::Poll);
    let store = FakeStore::new();
    let sleeper = RecordingSleeper { sleeps: RefCell::new(Vec::new()), events: None };

    let runtime = DeviceFlowRuntime::new(&api, &store, &sleeper);
    let result = complete_device_flow(&runtime, &login_request, |_| {});

    assert!(matches!(result, Err(DeviceFlowRunError::OAuth(FakeApiError::Poll))));
}

#[rstest]
fn store_errors_are_propagated(login_request: DeviceFlowLoginRequest) {
    let api =
        FakeApi::new(vec![TokenPollOutcome::Authorized(AccessToken::new("gho_secret", ["repo"]))]);
    let store = FakeStore::failing(FakeStoreError::Store);
    let sleeper = RecordingSleeper { sleeps: RefCell::new(Vec::new()), events: None };

    let runtime = DeviceFlowRuntime::new(&api, &store, &sleeper);
    let result = complete_device_flow(&runtime, &login_request, |_| {});

    assert!(matches!(result, Err(DeviceFlowRunError::Storage(FakeStoreError::Store))));
}

#[fixture]
fn login_request() -> DeviceFlowLoginRequest {
    DeviceFlowLoginRequest { client_id: ClientId::new("client"), scopes: vec!["repo".into()] }
}

#[fixture]
fn recording_sleeper() -> RecordingSleeper {
    RecordingSleeper { sleeps: RefCell::new(Vec::new()), events: None }
}
