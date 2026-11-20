//! Unit tests for the command-line surface.
//!
//! Every case here is answered by the usage text or rejected before a request
//! is attempted, so no test reaches GitHub. A successful listing run is
//! covered end to end against a loopback server in `repository_listing_e2e`.
//!
//! `fixtures` holds the shared harness, `usage` covers the argument vector and
//! the usage text, and `streams` covers standard input, the credential store,
//! and output streams that fail while writing.

mod fixtures;
mod streams;
mod usage;
