//! Filesystem and gRPC adapters for Qdrant liveness validation.

use std::{future::Future, io, time::Duration};

use camino::Utf8PathBuf;
use cap_std::{ambient_authority, fs_utf8::Dir};
use qdrant_client::{Qdrant, QdrantError, qdrant::HealthCheckReply};

use super::{
    DEFAULT_QDRANT_GRPC_ENDPOINT, QdrantApiKey, QdrantLivenessConfig, QdrantLivenessError,
    QdrantLivenessReport,
};

/// Runs the authenticated Qdrant health and collection probes once.
/// For example, a configured endpoint and key produce a readiness report only when both probes
/// succeed.
pub(super) async fn check_qdrant_liveness_once(
    config: &QdrantLivenessConfig,
) -> Result<QdrantLivenessReport, QdrantLivenessError> {
    let client = build_qdrant_client(config, config.api_key())?;

    let reply = timed_probe(config.timeout(), client.health_check()).await?;
    timed_probe(config.timeout(), client.list_collections()).await?;

    QdrantLivenessReport::try_from(reply)
}

/// Runs one Qdrant request future with the supplied timeout.
/// For example, an elapsed deadline becomes `Timeout`, while a completed client error is mapped to
/// the liveness error model.
async fn timed_probe<T, F>(timeout: Duration, probe: F) -> Result<T, QdrantLivenessError>
where
    F: Future<Output = Result<T, QdrantError>>,
{
    tokio::time::timeout(timeout, probe)
        .await
        .map_err(|_elapsed| QdrantLivenessError::Timeout { timeout })?
        .map_err(map_qdrant_error)
}

/// Reads the provisioned API key from a UTF-8 path through a directory handle.
/// For example, a missing file becomes `MissingApiKeyFile`; other path or I/O failures become
/// `UnreadableApiKeyFile`.
pub(super) fn read_api_key_file(path: &camino::Utf8Path) -> Result<String, QdrantLivenessError> {
    let parent = path.parent().ok_or_else(|| invalid_api_key_path_error(path))?;
    let filename = path.file_name().ok_or_else(|| invalid_api_key_path_error(path))?;
    let directory = Dir::open_ambient_dir(parent, ambient_authority())
        .map_err(|source| map_api_key_read_error(path.to_path_buf(), source))?;

    directory
        .read_to_string(filename)
        .map_err(|source| map_api_key_read_error(path.to_path_buf(), source))
}

/// Builds the typed error used when the configured API-key path cannot identify a file.
/// For example, a path without a parent or filename is reported as an unreadable key file with an
/// invalid-input source.
fn invalid_api_key_path_error(path: &camino::Utf8Path) -> QdrantLivenessError {
    QdrantLivenessError::UnreadableApiKeyFile {
        path: path.to_path_buf(),
        source: io::Error::new(io::ErrorKind::InvalidInput, "API-key path must name a file"),
    }
}

/// Maps API-key file I/O failures while preserving the path and underlying cause.
/// For example, `NotFound` becomes `MissingApiKeyFile`, and other I/O kinds become
/// `UnreadableApiKeyFile`.
fn map_api_key_read_error(path: Utf8PathBuf, source: io::Error) -> QdrantLivenessError {
    if source.kind() == io::ErrorKind::NotFound {
        QdrantLivenessError::MissingApiKeyFile { path }
    } else {
        QdrantLivenessError::UnreadableApiKeyFile { path, source }
    }
}

/// Builds an authenticated Qdrant client from the liveness configuration and validated key.
/// For example, an invalid endpoint is returned as `InvalidEndpoint`; other client construction
/// failures use the shared gRPC error mapping.
pub(super) fn build_qdrant_client(
    config: &QdrantLivenessConfig,
    api_key: &QdrantApiKey,
) -> Result<Qdrant, QdrantLivenessError> {
    Qdrant::from_url(config.endpoint())
        .api_key(api_key.as_secret())
        .skip_compatibility_check()
        .build()
        .map_err(|error| map_qdrant_build_error(config.endpoint(), error))
}

/// Maps client-construction errors using the endpoint being configured.
/// For example, an invalid URI is associated with that endpoint, while other failures use the
/// common Qdrant error mapping.
fn map_qdrant_build_error(endpoint: &str, error: QdrantError) -> QdrantLivenessError {
    if matches!(error, QdrantError::InvalidUri(ref _source)) {
        QdrantLivenessError::InvalidEndpoint { endpoint: endpoint.to_owned() }
    } else {
        map_qdrant_error(error)
    }
}

/// Converts Qdrant client errors into the liveness error categories.
/// For example, authentication failures become `AuthenticationFailed`, while transport and other
/// response failures become `GrpcUnavailable`.
fn map_qdrant_error(error: QdrantError) -> QdrantLivenessError {
    match error {
        QdrantError::ResponseError { status }
        | QdrantError::ResourceExhaustedError { status, .. }
            if is_authentication_failure_status(&status.code().to_string(), status.message()) =>
        {
            QdrantLivenessError::AuthenticationFailed
        }
        QdrantError::ResponseError { status }
        | QdrantError::ResourceExhaustedError { status, .. } => {
            QdrantLivenessError::GrpcUnavailable {
                message: format!("{}: {}", status.code(), status.message()),
            }
        }
        QdrantError::InvalidUri(_source) => QdrantLivenessError::InvalidEndpoint {
            endpoint: DEFAULT_QDRANT_GRPC_ENDPOINT.to_owned(),
        },
        QdrantError::Io(source) => {
            QdrantLivenessError::GrpcUnavailable { message: source.kind().to_string() }
        }
        other => QdrantLivenessError::GrpcUnavailable { message: other.to_string() },
    }
}

/// Recognizes gRPC status codes that directly indicate failed authentication.
/// For example, `Unauthenticated` and `PermissionDenied` return `true`; unrelated codes return
/// `false`.
pub(super) fn is_authentication_failure_code(code: &str) -> bool {
    matches!(code, "Unauthenticated" | "PermissionDenied")
}

/// Checks both the gRPC status code and Qdrant error text for authentication failures.
/// For example, an authentication status or a recognized invalid-key message returns `true`.
pub(super) fn is_authentication_failure_status(code: &str, message: &str) -> bool {
    is_authentication_failure_code(code) || is_qdrant_authentication_failure_message(message)
}

/// Recognizes Qdrant message text that indicates rejected credentials.
/// For example, messages mentioning invalid API keys or JWTs match after case normalization.
fn is_qdrant_authentication_failure_message(message: &str) -> bool {
    let normalized_message = message.to_ascii_lowercase();

    normalized_message.contains("authentication credentials")
        || normalized_message.contains("invalid api key")
        || normalized_message.contains("invalid jwt")
}

impl TryFrom<HealthCheckReply> for QdrantLivenessReport {
    type Error = QdrantLivenessError;

    fn try_from(reply: HealthCheckReply) -> Result<Self, Self::Error> {
        if reply.version.is_empty() {
            return Err(QdrantLivenessError::MissingServerVersion);
        }

        Ok(Self::new(reply.title, reply.version, reply.commit))
    }
}
