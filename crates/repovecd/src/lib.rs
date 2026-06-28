//! Runtime adapters for the repovec control-plane daemon.
//!
//! This crate owns side-effecting runtime code for `repovecd`: HTTP clients,
//! filesystem access, command execution, and application orchestration. Pure
//! domain validation and policy stay in `repovec-core`; I/O enters through
//! small call-boundary adapters here.
//!
//! The `github_oauth_client` module implements the blocking HTTP client for
//! GitHub-compatible OAuth device-flow endpoints. The `github_token_store`
//! module persists encrypted GitHub tokens through the `systemd-creds` adapter
//! boundary. The `github_device_flow` module ties those ports together to run
//! the RFC 8628 flow until a token is stored or a terminal outcome is reached.
//! Intended consumers are `repovecd::main`, runnable examples, and integration
//! tests that need the daemon's runtime boundary without duplicating adapter
//! code.

pub mod github_device_flow;
pub mod github_oauth_client;
pub mod github_token_store;

#[cfg(test)]
mod tracing_test;
