//! Observation and assertions for the one-shot listing composition.
//!
//! A composition run produces a status and two streams. Every end-to-end test
//! asserts the same three promises about a run — its status, whether it wrote
//! observations, and what its diagnostic explains — so those promises live here
//! in the language of the contract rather than repeated as raw assertions.

use std::{error::Error, io::Read};

use repovec_test_helpers::recording_http::RecordingServer;
use repovecd::repository_listing_command::{ListingMode, RunStatus, UserTokenLoader};

use crate::listing_composition::run_buffered;

/// What one composition run wrote, as an operator would see it.
#[derive(Debug)]
pub struct Observation {
    /// Status the composition reported for the run.
    status: RunStatus,
    /// Everything the run wrote to standard output.
    pub stdout: String,
    /// Everything the run wrote to standard error.
    pub stderr: String,
}

/// Runs the composition and captures its status and streams.
///
/// # Errors
///
/// Returns the fixture's construction or decoding failure rather than
/// panicking, so a broken fixture is reported as a failed test.
pub fn observe(
    server: &RecordingServer,
    mode: ListingMode,
    stdin: &mut dyn Read,
    loader: &dyn UserTokenLoader,
) -> Result<Observation, Box<dyn Error>> {
    let (status, stdout, stderr) = run_buffered(server, mode, stdin, loader)?;
    Ok(Observation {
        status,
        stdout,
        stderr,
    })
}

impl Observation {
    /// Asserts the run succeeded and wrote exactly the expected observations.
    pub fn assert_succeeded(&self, expected: &str) {
        assert_eq!(
            self.status,
            RunStatus::Success,
            "a completed traversal must succeed"
        );
        assert_eq!(
            self.stdout, expected,
            "every observation must be written in page order"
        );
        assert!(
            self.stderr.is_empty(),
            "a successful run must write no diagnostic"
        );
    }

    /// Asserts the run failed as expected and wrote no observation.
    ///
    /// The diagnostic must name the cause the run has in common with its
    /// class, so a failure cannot pass by reporting only its status.
    pub fn assert_refused(&self, expected: RunStatus, cause: &str) {
        assert_eq!(
            self.status, expected,
            "the run must report its documented failure class"
        );
        assert!(
            self.stdout.is_empty(),
            "a failed run must write no observation"
        );
        assert!(
            self.stderr.contains(cause),
            "the diagnostic must explain the failure with {cause:?}: {}",
            self.stderr
        );
    }

    /// Asserts neither stream reveals the supplied secret.
    pub fn assert_hides(&self, secret: &str) {
        assert!(
            !self.stdout.contains(secret) && !self.stderr.contains(secret),
            "no stream may reveal the credential"
        );
    }
}
