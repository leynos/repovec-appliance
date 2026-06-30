//! Error types for Qdrant liveness validation.

use std::time::Duration;

use camino::Utf8PathBuf;

/// Errors returned while proving Qdrant runtime liveness.
#[derive(Debug, thiserror::Error)]
pub enum QdrantLivenessError {
    /// The API-key file does not exist.
    #[error("Qdrant API-key file is missing: {path}")]
    MissingApiKeyFile {
        /// The configured API-key file path.
        path: Utf8PathBuf,
    },
    /// The API-key file exists but cannot be read.
    #[error("Qdrant API-key file cannot be read: {path}")]
    UnreadableApiKeyFile {
        /// The configured API-key file path.
        path: Utf8PathBuf,
        /// The underlying filesystem error.
        #[source]
        source: std::io::Error,
    },
    /// The API-key file is empty.
    #[error("Qdrant API key is empty")]
    EmptyApiKey,
    /// The API key cannot be represented as gRPC metadata.
    #[error("Qdrant API key is not a valid gRPC metadata value: <redacted>")]
    InvalidApiKey,
    /// The configured Qdrant endpoint cannot be parsed.
    #[error("Qdrant gRPC endpoint is invalid: {endpoint}")]
    InvalidEndpoint {
        /// The configured endpoint URI.
        endpoint: String,
    },
    /// The liveness check did not complete within the configured timeout.
    #[error("Qdrant liveness check timed out after {timeout:?}")]
    Timeout {
        /// The timeout that elapsed.
        timeout: Duration,
    },
    /// Qdrant rejected the supplied API key.
    #[error("Qdrant rejected the configured API key")]
    AuthenticationFailed,
    /// Qdrant could not be reached over gRPC.
    #[error("Qdrant gRPC liveness check failed: {message}")]
    GrpcUnavailable {
        /// A redacted diagnostic from the gRPC client.
        message: String,
    },
    /// Qdrant responded but did not provide readiness metadata.
    #[error("Qdrant health reply did not include a server version")]
    MissingServerVersion,
}
