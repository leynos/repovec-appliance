//! Behavioural tests for GitHub OAuth device-flow policy.

use std::time::Duration;

use oauth2::{DeviceCodeErrorResponseType, basic::BasicErrorResponseType};
use repovec_core::github_oauth::{
    AccessToken, PollDecision, TerminalDeviceFlowError, TokenPollOutcome, apply_poll_outcome,
    classify_device_code_error,
};
use rstest::fixture;
use rstest_bdd_macros::{given, scenarios, then, when};

#[derive(Default)]
struct GitHubOAuthWorld {
    active_interval: Duration,
    outcome: Option<TokenPollOutcome>,
    decision: Option<PollDecision>,
    oauth_error: Option<DeviceCodeErrorResponseType>,
    has_classification: bool,
    classified_outcome: Option<TokenPollOutcome>,
}

#[fixture]
fn github_oauth_world() -> GitHubOAuthWorld {
    let active_interval = Duration::ZERO;
    GitHubOAuthWorld { active_interval, ..Default::default() }
}

#[given("the active polling interval is 5 seconds")]
fn the_active_polling_interval_is_5_seconds(github_oauth_world: &mut GitHubOAuthWorld) {
    github_oauth_world.active_interval = Duration::from_secs(5);
}

#[given("the OAuth error is temporarily_unavailable")]
fn the_oauth_error_is_temporarily_unavailable(github_oauth_world: &mut GitHubOAuthWorld) {
    github_oauth_world.oauth_error = Some(DeviceCodeErrorResponseType::Basic(
        BasicErrorResponseType::Extension("temporarily_unavailable".to_owned()),
    ));
}

#[when("the token endpoint returns an access token")]
fn the_token_endpoint_returns_an_access_token(github_oauth_world: &mut GitHubOAuthWorld) {
    github_oauth_world.outcome =
        Some(TokenPollOutcome::Authorized(AccessToken::new("gho_secret", ["repo"])));
    apply_outcome(github_oauth_world);
}

#[when("the token endpoint asks the client to slow down")]
fn the_token_endpoint_asks_the_client_to_slow_down(github_oauth_world: &mut GitHubOAuthWorld) {
    github_oauth_world.outcome = Some(TokenPollOutcome::SlowDown);
    apply_outcome(github_oauth_world);
}

#[when("the token endpoint reports access denied")]
fn the_token_endpoint_reports_access_denied(github_oauth_world: &mut GitHubOAuthWorld) {
    github_oauth_world.outcome = Some(TokenPollOutcome::AccessDenied);
    apply_outcome(github_oauth_world);
}

#[when("the token endpoint reports an expired token")]
fn the_token_endpoint_reports_an_expired_token(github_oauth_world: &mut GitHubOAuthWorld) {
    github_oauth_world.outcome = Some(TokenPollOutcome::ExpiredToken);
    apply_outcome(github_oauth_world);
}

#[when("the OAuth error is classified")]
fn the_oauth_error_is_classified(github_oauth_world: &mut GitHubOAuthWorld) {
    let Some(error) = github_oauth_world.oauth_error.as_ref() else {
        panic!("an OAuth error should be supplied");
    };
    github_oauth_world.classified_outcome = classify_device_code_error(error);
    github_oauth_world.has_classification = true;
}

#[then("polling completes successfully")]
fn polling_completes_successfully(github_oauth_world: &GitHubOAuthWorld) {
    assert!(matches!(github_oauth_world.decision, Some(PollDecision::Complete(_))));
}

#[then("the next polling interval is 10 seconds")]
fn the_next_polling_interval_is_10_seconds(github_oauth_world: &GitHubOAuthWorld) {
    assert_eq!(
        github_oauth_world.decision,
        Some(PollDecision::Continue { next_interval: Duration::from_secs(10) }),
    );
}

#[then("polling fails because access was denied")]
fn polling_fails_because_access_was_denied(github_oauth_world: &GitHubOAuthWorld) {
    assert_eq!(
        github_oauth_world.decision,
        Some(PollDecision::Failed(TerminalDeviceFlowError::AccessDenied)),
    );
}

#[then("polling fails because the device code expired")]
fn polling_fails_because_the_device_code_expired(github_oauth_world: &GitHubOAuthWorld) {
    assert_eq!(
        github_oauth_world.decision,
        Some(PollDecision::Failed(TerminalDeviceFlowError::ExpiredToken)),
    );
}

#[then("no device-flow polling outcome is produced")]
fn no_device_flow_polling_outcome_is_produced(github_oauth_world: &GitHubOAuthWorld) {
    assert!(github_oauth_world.has_classification);
    assert_eq!(github_oauth_world.classified_outcome, None);
}

fn apply_outcome(github_oauth_world: &mut GitHubOAuthWorld) {
    let Some(outcome) = github_oauth_world.outcome.take() else {
        panic!("a token endpoint outcome should be supplied");
    };
    github_oauth_world.decision =
        Some(apply_poll_outcome(outcome, github_oauth_world.active_interval));
}

scenarios!("tests/features/github_oauth.feature", fixtures = [github_oauth_world: GitHubOAuthWorld]);
