//! Shared sequences for Qdrant liveness unit tests.
//!
//! The daemon-startup and liveness-startup suites both need the same
//! unavailable-then-healthy probe sequence. Keeping one copy here prevents the
//! two retry stories from drifting apart while staying inside the crate, so
//! both suites see the same [`QdrantLivenessError`] type.

use super::QdrantLivenessError;

/// Produces a deterministic unavailable-then-healthy Qdrant liveness sequence.
///
/// Attempt `0` returns [`QdrantLivenessError::GrpcUnavailable`] and every later
/// attempt succeeds, so a retry test observes exactly one transient failure
/// followed by recovery.
///
/// # Errors
///
/// Returns `Err(QdrantLivenessError::GrpcUnavailable)` for attempt `0`.
pub(crate) fn transient_qdrant_result(attempt: i32) -> Result<(), QdrantLivenessError> {
    match attempt {
        0 => Err(QdrantLivenessError::GrpcUnavailable {
            message: String::from("connection refused"),
        }),
        _ => Ok(()),
    }
}
