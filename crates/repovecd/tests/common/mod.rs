//! Shared support for the repository listing integration tests.
//!
//! Each helper lives in its own module so a test binary that needs only one of
//! them does not leave the others as dead code; the workspace forbids silencing
//! that with an allowance. Store fixtures, credentials, and request builders
//! live in `repovec_test_helpers::repository_listing_wire`, and the recording
//! server in `repovec_test_helpers::recording_http`.

pub mod blocking_client;
pub mod installation_client;
pub mod user_client;
