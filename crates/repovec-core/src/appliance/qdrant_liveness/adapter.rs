//! Filesystem and gRPC adapters for Qdrant liveness validation.

use std::{future::Future, io, time::Duration};

use camino::Utf8PathBuf;
use cap_std::{ambient_authority, fs_utf8::Dir};
use qdrant_client::{Qdrant, QdrantError, qdrant::HealthCheckReply};

use super::{
    DEFAULT_QDRANT_GRPC_ENDPOINT, QdrantApiKey, QdrantLivenessConfig, QdrantLivenessError,
    QdrantLivenessReport, load_qdrant_api_key,
};

pub(super) async fn check_qdrant_liveness_once(
    config: &QdrantLivenessConfig,
) -> Result<QdrantLivenessReport, QdrantLivenessError> {
    let api_key = load_qdrant_api_key(config)?;
    let client = build_qdrant_client(config, &api_key)?;

    let reply = timed_probe(config.timeout(), client.health_check()).await?;
    timed_probe(config.timeout(), client.list_collections()).await?;

    QdrantLivenessReport::try_from(reply)
}

async fn timed_probe<T, F>(timeout: Duration, probe: F) -> Result<T, QdrantLivenessError>
where
    F: Future<Output = Result<T, QdrantError>>,
{
    tokio::time::timeout(timeout, probe)
        .await
        .map_err(|_elapsed| QdrantLivenessError::Timeout { timeout })?
        .map_err(map_qdrant_error)
}

pub(super) fn read_api_key_file(path: &camino::Utf8Path) -> Result<String, QdrantLivenessError> {
    let parent = path.parent().ok_or_else(|| invalid_api_key_path_error(path))?;
    let filename = path.file_name().ok_or_else(|| invalid_api_key_path_error(path))?;
    let directory = Dir::open_ambient_dir(parent, ambient_authority())
        .map_err(|source| map_api_key_read_error(path.to_path_buf(), source))?;

    directory
        .read_to_string(filename)
        .map_err(|source| map_api_key_read_error(path.to_path_buf(), source))
}

fn invalid_api_key_path_error(path: &camino::Utf8Path) -> QdrantLivenessError {
    QdrantLivenessError::UnreadableApiKeyFile {
        path: path.to_path_buf(),
        source: io::Error::new(io::ErrorKind::InvalidInput, "API-key path must name a file"),
    }
}

fn map_api_key_read_error(path: Utf8PathBuf, source: io::Error) -> QdrantLivenessError {
    if source.kind() == io::ErrorKind::NotFound {
        QdrantLivenessError::MissingApiKeyFile { path }
    } else {
        QdrantLivenessError::UnreadableApiKeyFile { path, source }
    }
}

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

fn map_qdrant_build_error(endpoint: &str, error: QdrantError) -> QdrantLivenessError {
    if matches!(error, QdrantError::InvalidUri(ref _source)) {
        QdrantLivenessError::InvalidEndpoint { endpoint: endpoint.to_owned() }
    } else {
        map_qdrant_error(error)
    }
}

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

pub(super) fn is_authentication_failure_code(code: &str) -> bool {
    matches!(code, "Unauthenticated" | "PermissionDenied")
}

pub(super) fn is_authentication_failure_status(code: &str, message: &str) -> bool {
    is_authentication_failure_code(code) || is_qdrant_authentication_failure_message(message)
}

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
