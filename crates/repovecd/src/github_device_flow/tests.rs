//! Tests for device-flow orchestration.

use std::{cell::RefCell, convert::Infallible, rc::Rc, time::Duration};

use repovec_core::github_oauth::{AccessToken, ClientId, DeviceCode, TokenPollOutcome, UserCode};
use rstest::{fixture, rstest};

use super::{
    DeviceAuthorization, DeviceFlowApi, DeviceFlowLoginRequest, DeviceFlowRunError,
    DeviceFlowRuntime, Sleeper, TerminalDeviceFlowError, TokenStore, complete_device_flow,
};

struct FakeApi {
    outcomes: RefCell<Vec<TokenPollOutcome>>,
    expires_in: Duration,
    events: Option<Rc<RefCell<Vec<&'static str>>>>,
}

impl DeviceFlowApi for FakeApi {
    type Error = Infallible;

    fn request_device_code(
        &self,
        _client_id: &ClientId,
        _scopes: &[String],
    ) -> Result<DeviceAuthorization, Self::Error> {
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
        Ok(self.outcomes.borrow_mut().remove(0))
    }
}

struct FakeStore {
    tokens: RefCell<Vec<AccessToken>>,
}

impl TokenStore for FakeStore {
    type Error = Infallible;

    fn store(&self, token: &AccessToken) -> Result<(), Self::Error> {
        self.tokens.borrow_mut().push(token.clone());
        Ok(())
    }
}

struct RecordingSleeper {
    sleeps: RefCell<Vec<Duration>>,
    elapsed: RefCell<Duration>,
    events: Option<Rc<RefCell<Vec<&'static str>>>>,
}

impl Sleeper for RecordingSleeper {
    fn sleep(&self, duration: Duration) {
        if let Some(events) = &self.events {
            events.borrow_mut().push("sleep");
        }
        self.sleeps.borrow_mut().push(duration);
        *self.elapsed.borrow_mut() += duration;
    }

    fn elapsed(&self) -> Duration { *self.elapsed.borrow() }
}

#[rstest]
fn happy_path_stores_the_authorized_token(
    login_request: DeviceFlowLoginRequest,
    recording_sleeper: RecordingSleeper,
) {
    let api = FakeApi {
        outcomes: RefCell::new(vec![
            TokenPollOutcome::AuthorizationPending,
            TokenPollOutcome::Authorized(AccessToken::new("gho_secret", ["repo"])),
        ]),
        expires_in: Duration::from_secs(900),
        events: None,
    };
    let store = FakeStore { tokens: RefCell::new(Vec::new()) };

    let runtime = DeviceFlowRuntime::new(&api, &store, &recording_sleeper);
    let result = complete_device_flow(&runtime, &login_request, |_| {})
        .expect("device flow should complete");

    assert_eq!(result.prompt.user_code.as_str(), "ABCD-1234");
    assert_eq!(store.tokens.borrow().len(), 1);
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
    let api = FakeApi {
        outcomes: RefCell::new(vec![
            TokenPollOutcome::SlowDown,
            TokenPollOutcome::Authorized(AccessToken::new("gho_secret", ["repo"])),
        ]),
        expires_in: Duration::from_secs(900),
        events: None,
    };
    let store = FakeStore { tokens: RefCell::new(Vec::new()) };

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
    let api = FakeApi {
        outcomes: RefCell::new(vec![TokenPollOutcome::AccessDenied]),
        expires_in: Duration::from_secs(900),
        events: None,
    };
    let store = FakeStore { tokens: RefCell::new(Vec::new()) };

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
    let api = FakeApi {
        outcomes: RefCell::new(vec![TokenPollOutcome::AuthorizationPending]),
        expires_in: Duration::from_secs(4),
        events: None,
    };
    let store = FakeStore { tokens: RefCell::new(Vec::new()) };

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
    let api = FakeApi {
        outcomes: RefCell::new(vec![TokenPollOutcome::AccessDenied]),
        expires_in: Duration::from_secs(900),
        events: Some(Rc::clone(&events)),
    };
    let store = FakeStore { tokens: RefCell::new(Vec::new()) };
    let sleeper = RecordingSleeper {
        sleeps: RefCell::new(Vec::new()),
        elapsed: RefCell::new(Duration::ZERO),
        events: Some(Rc::clone(&events)),
    };

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

#[fixture]
fn login_request() -> DeviceFlowLoginRequest {
    DeviceFlowLoginRequest { client_id: ClientId::new("client"), scopes: vec!["repo".into()] }
}

#[fixture]
fn recording_sleeper() -> RecordingSleeper {
    RecordingSleeper {
        sleeps: RefCell::new(Vec::new()),
        elapsed: RefCell::new(Duration::ZERO),
        events: None,
    }
}
