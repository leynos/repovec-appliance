//! Tests for GitHub OAuth device-flow policy.

use std::time::Duration;

use oauth2::{DeviceCodeErrorResponseType, basic::BasicErrorResponseType};
use proptest::prelude::*;
use rstest::rstest;

use super::{
    AccessToken, DeviceCode, PollDecision, SLOW_DOWN_EXTRA_DELAY, TerminalDeviceFlowError,
    TokenPollOutcome, UserCode, apply_poll_outcome, classify_device_code_error,
};

#[rstest]
#[case(
    DeviceCodeErrorResponseType::AuthorizationPending,
    Some(TokenPollOutcome::AuthorizationPending)
)]
#[case(DeviceCodeErrorResponseType::SlowDown, Some(TokenPollOutcome::SlowDown))]
#[case(DeviceCodeErrorResponseType::AccessDenied, Some(TokenPollOutcome::AccessDenied))]
#[case(DeviceCodeErrorResponseType::ExpiredToken, Some(TokenPollOutcome::ExpiredToken))]
#[case(
    DeviceCodeErrorResponseType::Basic(BasicErrorResponseType::Extension("temporarily_unavailable".into())),
    None
)]
fn device_code_errors_are_classified(
    #[case] error: DeviceCodeErrorResponseType,
    #[case] expected: Option<TokenPollOutcome>,
) {
    assert_eq!(classify_device_code_error(&error), expected);
}

#[test]
fn authorization_pending_keeps_the_active_interval() {
    let interval = Duration::from_secs(7);

    let decision = apply_poll_outcome(TokenPollOutcome::AuthorizationPending, interval);

    assert_eq!(decision, PollDecision::Continue { next_interval: interval });
}

#[test]
fn slow_down_increases_the_active_interval() {
    let interval = Duration::from_secs(7);

    let decision = apply_poll_outcome(TokenPollOutcome::SlowDown, interval);

    assert_eq!(
        decision,
        PollDecision::Continue { next_interval: interval + SLOW_DOWN_EXTRA_DELAY },
    );
}

#[rstest]
#[case(TokenPollOutcome::AccessDenied, TerminalDeviceFlowError::AccessDenied)]
#[case(TokenPollOutcome::ExpiredToken, TerminalDeviceFlowError::ExpiredToken)]
fn terminal_outcomes_stop_polling(
    #[case] outcome: TokenPollOutcome,
    #[case] expected: TerminalDeviceFlowError,
) {
    let decision = apply_poll_outcome(outcome, Duration::from_secs(5));

    assert_eq!(decision, PollDecision::Failed(expected));
}

#[test]
fn authorized_outcome_returns_redacted_token() {
    let token = AccessToken::new("gho_secret", ["repo"]);

    let decision = apply_poll_outcome(TokenPollOutcome::Authorized(token), Duration::from_secs(5));

    assert!(matches!(decision, PollDecision::Complete(_)));
    assert!(!format!("{decision:?}").contains("gho_secret"));
}

#[test]
fn secret_debug_output_is_redacted() {
    let token = AccessToken::new("gho_secret", ["repo"]);

    assert!(!format!("{:?}", DeviceCode::new("device-secret")).contains("device-secret"));
    assert!(!format!("{:?}", UserCode::new("ABCD-1234")).contains("ABCD-1234"));
    assert!(!format!("{token:?}").contains("gho_secret"));
}

proptest! {
    #[test]
    fn pending_never_reduces_the_poll_interval(seconds in 0_u64..86_400) {
        let interval = Duration::from_secs(seconds);

        let decision = apply_poll_outcome(TokenPollOutcome::AuthorizationPending, interval);

        prop_assert_eq!(decision, PollDecision::Continue { next_interval: interval });
    }

    #[test]
    fn slow_down_monotonically_increases_the_poll_interval(seconds in 0_u64..86_400) {
        let interval = Duration::from_secs(seconds);

        let decision = apply_poll_outcome(TokenPollOutcome::SlowDown, interval);

        match decision {
            PollDecision::Continue { next_interval } => {
                prop_assert!(next_interval >= interval + SLOW_DOWN_EXTRA_DELAY);
            }
            unexpected => prop_assert!(false, "unexpected decision: {unexpected:?}"),
        }
    }
}
