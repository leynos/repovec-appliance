//! Test doubles shared by the device-flow orchestration tests.

use std::{cell::RefCell, rc::Rc, time::Duration};

use repovec_core::github_oauth::{AccessToken, ClientId, DeviceCode, TokenPollOutcome, UserCode};
use thiserror::Error;

use super::super::{DeviceAuthorization, DeviceFlowApi, Sleeper, TokenStore};

pub(super) struct FakeApi {
    outcomes: RefCell<Vec<TokenPollOutcome>>,
    expires_in: Duration,
    events: Option<Rc<RefCell<Vec<&'static str>>>>,
    request_error: Option<FakeApiError>,
    poll_error: Option<FakeApiError>,
}

impl FakeApi {
    pub(super) fn new(outcomes: Vec<TokenPollOutcome>) -> Self {
        Self {
            outcomes: RefCell::new(outcomes),
            expires_in: Duration::from_secs(900),
            events: None,
            request_error: None,
            poll_error: None,
        }
    }

    pub(super) fn with_expires_in(mut self, expires_in: Duration) -> Self {
        self.expires_in = expires_in;
        self
    }

    pub(super) fn with_events(mut self, events: Rc<RefCell<Vec<&'static str>>>) -> Self {
        self.events = Some(events);
        self
    }

    pub(super) fn request_fails(error: FakeApiError) -> Self {
        Self {
            request_error: Some(error),
            ..Self::new(Vec::new())
        }
    }

    pub(super) fn poll_fails(error: FakeApiError) -> Self {
        Self {
            poll_error: Some(error),
            ..Self::new(Vec::new())
        }
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
pub(super) enum FakeApiError {
    #[error("request failed")]
    Request,
    #[error("poll failed")]
    Poll,
}

pub(super) struct FakeStore {
    pub(super) tokens: RefCell<Vec<AccessToken>>,
    error: Option<FakeStoreError>,
}

impl FakeStore {
    pub(super) fn new() -> Self {
        Self {
            tokens: RefCell::new(Vec::new()),
            error: None,
        }
    }

    pub(super) fn failing(error: FakeStoreError) -> Self {
        Self {
            tokens: RefCell::new(Vec::new()),
            error: Some(error),
        }
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
pub(super) enum FakeStoreError {
    #[error("store failed")]
    Store,
}

pub(super) struct RecordingSleeper {
    pub(super) sleeps: RefCell<Vec<Duration>>,
    pub(super) events: Option<Rc<RefCell<Vec<&'static str>>>>,
}

impl Sleeper for RecordingSleeper {
    fn sleep(&self, duration: Duration) {
        if let Some(events) = &self.events {
            events.borrow_mut().push("sleep");
        }
        self.sleeps.borrow_mut().push(duration);
    }
}
