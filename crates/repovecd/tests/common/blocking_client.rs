//! HTTP client construction shared by the listing adapter fixtures.
//!
//! Every fixture client disables redirects and bounds its deadlines, so a test
//! whose scripted server stops responding fails promptly instead of hanging
//! the suite.

use std::time::Duration;

use reqwest::blocking::Client;

/// The connection deadline applied to every test client.
///
/// Tests never dial an unreachable host, so this only bounds a loopback
/// connection that the server has stopped accepting.
const CONNECT_TIMEOUT: Duration = Duration::from_millis(500);

/// Builds a blocking client with redirects disabled and the given deadline.
///
/// Redirects are disabled because following one would move a request without
/// the continuation checks seeing it; the adapter's production client is built
/// the same way, and a test asserts the resulting behaviour.
///
/// # Errors
///
/// Returns [`std::io::Error`] when the TLS backend cannot be initialized. The
/// caller reports it, so a broken test environment never looks like a failed
/// assertion.
pub fn blocking_client(timeout: Duration) -> std::io::Result<Client> {
    Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .connect_timeout(CONNECT_TIMEOUT)
        .timeout(timeout)
        .build()
        .map_err(std::io::Error::other)
}
