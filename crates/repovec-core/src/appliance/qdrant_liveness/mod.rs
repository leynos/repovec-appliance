//! Runtime liveness checks for the local Qdrant service.
//!
//! The static [`crate::appliance::qdrant_quadlet`] validator proves that the
//! checked-in Quadlet is wired correctly. This module proves the runtime half
//! of the same appliance contract: local Rust callers can load the stored API
//! key, connect to Qdrant's gRPC endpoint, and receive non-secret readiness
//! evidence from the service.

use std::{fmt, io, time::Duration};

use camino::Utf8PathBuf;
use cap_std::{ambient_authority, fs_utf8::Dir};
use qdrant_client::{Qdrant, QdrantError, qdrant::HealthCheckReply};

mod error;
mod observability;
mod startup;

pub use error::QdrantLivenessError;
pub use observability::qdrant_liveness_error_category;
use observability::{qdrant_liveness_span, record_qdrant_liveness_result};
pub use startup::{QdrantStartupLivenessPolicy, wait_for_qdrant_startup_liveness};

/// Qdrant's appliance gRPC endpoint.
pub const DEFAULT_QDRANT_GRPC_ENDPOINT: &str = "http://127.0.0.1:6334";

/// Location populated by the Qdrant API-key provisioning service.
pub const DEFAULT_QDRANT_API_KEY_PATH: &str = "/etc/repovec/qdrant-api-key";

/// Maximum time spent waiting for one Qdrant liveness probe by default.
pub const DEFAULT_QDRANT_LIVENESS_TIMEOUT: Duration = Duration::from_secs(5);

/// Configuration for the Qdrant runtime liveness probe.
///
/// # Examples
///
/// ```no_run
/// use repovec_core::appliance::qdrant_liveness::{
///     DEFAULT_QDRANT_GRPC_ENDPOINT, QdrantLivenessConfig,
/// };
///
/// let config = QdrantLivenessConfig::default();
///
/// assert_eq!(config.endpoint(), DEFAULT_QDRANT_GRPC_ENDPOINT);
/// ```
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct QdrantLivenessConfig {
    endpoint: String,
    api_key_path: Utf8PathBuf,
    timeout: Duration,
}

impl QdrantLivenessConfig {
    /// Creates a liveness configuration from explicit values.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use std::time::Duration;
    ///
    /// use camino::Utf8PathBuf;
    /// use repovec_core::appliance::qdrant_liveness::QdrantLivenessConfig;
    ///
    /// let config = QdrantLivenessConfig::new(
    ///     "http://127.0.0.1:6334",
    ///     Utf8PathBuf::from("/tmp/qdrant-key"),
    ///     Duration::from_secs(2),
    /// );
    ///
    /// assert_eq!(config.timeout(), Duration::from_secs(2));
    /// ```
    #[must_use]
    pub fn new(endpoint: impl Into<String>, api_key_path: Utf8PathBuf, timeout: Duration) -> Self {
        Self { endpoint: endpoint.into(), api_key_path, timeout }
    }

    /// Returns the Qdrant gRPC endpoint URI.
    #[must_use]
    pub fn endpoint(&self) -> &str { &self.endpoint }

    /// Returns the file path from which the API key is loaded.
    #[must_use]
    pub fn api_key_path(&self) -> &camino::Utf8Path { self.api_key_path.as_path() }

    /// Returns the per-probe timeout.
    #[must_use]
    pub const fn timeout(&self) -> Duration { self.timeout }
}

impl Default for QdrantLivenessConfig {
    fn default() -> Self {
        Self::new(
            DEFAULT_QDRANT_GRPC_ENDPOINT,
            Utf8PathBuf::from(DEFAULT_QDRANT_API_KEY_PATH),
            DEFAULT_QDRANT_LIVENESS_TIMEOUT,
        )
    }
}

/// Non-secret readiness evidence returned by Qdrant's gRPC health endpoint.
///
/// # Examples
///
/// ```no_run
/// use repovec_core::appliance::qdrant_liveness::QdrantLivenessReport;
///
/// let report = QdrantLivenessReport::new("qdrant", "1.15.0", Some("abc123"));
///
/// assert_eq!(report.title(), "qdrant");
/// ```
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct QdrantLivenessReport {
    title: String,
    version: String,
    commit: Option<String>,
}

impl QdrantLivenessReport {
    /// Creates readiness evidence from Qdrant server metadata.
    #[must_use]
    pub fn new(
        title: impl Into<String>,
        version: impl Into<String>,
        commit: Option<impl Into<String>>,
    ) -> Self {
        Self { title: title.into(), version: version.into(), commit: commit.map(Into::into) }
    }

    /// Returns Qdrant's server title.
    #[must_use]
    pub fn title(&self) -> &str { &self.title }

    /// Returns Qdrant's server version.
    #[must_use]
    pub fn version(&self) -> &str { &self.version }

    /// Returns the optional Qdrant server commit identifier.
    #[must_use]
    pub fn commit(&self) -> Option<&str> { self.commit.as_deref() }
}

/// Validated Qdrant API key material.
#[derive(Clone, Eq, PartialEq)]
pub struct QdrantApiKey(String);

impl QdrantApiKey {
    /// Validates API-key material for use as gRPC metadata.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use repovec_core::appliance::qdrant_liveness::QdrantApiKey;
    ///
    /// let key = QdrantApiKey::parse("0123456789abcdef").unwrap();
    ///
    /// assert_eq!(key.as_secret(), "0123456789abcdef");
    /// ```
    ///
    /// # Errors
    ///
    /// Returns [`QdrantLivenessError::EmptyApiKey`] for empty input and
    /// [`QdrantLivenessError::InvalidApiKey`] for values that cannot be sent
    /// as a gRPC metadata value.
    pub fn parse(raw_value: impl Into<String>) -> Result<Self, QdrantLivenessError> {
        let value = raw_value.into();
        if value.is_empty() {
            return Err(QdrantLivenessError::EmptyApiKey);
        }
        if value.bytes().any(is_invalid_metadata_value_byte) {
            return Err(QdrantLivenessError::InvalidApiKey);
        }
        Ok(Self(value))
    }

    /// Exposes the secret to the gRPC adapter.
    #[must_use]
    pub fn as_secret(&self) -> &str { &self.0 }
}

impl fmt::Debug for QdrantApiKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("QdrantApiKey").field(&"<redacted>").finish()
    }
}

const fn is_invalid_metadata_value_byte(byte: u8) -> bool { !matches!(byte, 0x20..=0x7e) }

/// Loads and validates the Qdrant API key from the configured file path.
///
/// # Examples
///
/// ```no_run
/// use repovec_core::appliance::qdrant_liveness::{
///     QdrantLivenessConfig, load_qdrant_api_key,
/// };
///
/// let key = load_qdrant_api_key(&QdrantLivenessConfig::default())?;
///
/// assert!(!key.as_secret().is_empty());
/// # Ok::<(), repovec_core::appliance::qdrant_liveness::QdrantLivenessError>(())
/// ```
///
/// # Errors
///
/// Returns [`QdrantLivenessError`] when the configured key file is missing,
/// unreadable, empty, or unsuitable for gRPC metadata.
pub fn load_qdrant_api_key(
    config: &QdrantLivenessConfig,
) -> Result<QdrantApiKey, QdrantLivenessError> {
    let contents = read_api_key_file(config.api_key_path())?;
    QdrantApiKey::parse(contents)
}

/// Connects to Qdrant over gRPC and confirms that it reports healthy.
///
/// # Examples
///
/// ```no_run
/// # async fn example()
/// # -> Result<(), repovec_core::appliance::qdrant_liveness::QdrantLivenessError> {
/// use repovec_core::appliance::qdrant_liveness::{
///     QdrantLivenessConfig, check_qdrant_liveness,
/// };
///
/// let report = check_qdrant_liveness(&QdrantLivenessConfig::default()).await?;
///
/// assert!(!report.version().is_empty());
/// # Ok(())
/// # }
/// ```
///
/// # Errors
///
/// Returns [`QdrantLivenessError`] when the API key cannot be loaded, the
/// endpoint is invalid, Qdrant is unreachable, authentication fails, the probe
/// times out, or the health reply does not contain readiness metadata.
pub async fn check_qdrant_liveness(
    config: &QdrantLivenessConfig,
) -> Result<QdrantLivenessReport, QdrantLivenessError> {
    let span = qdrant_liveness_span(config);
    let result = check_qdrant_liveness_once(config).await;
    record_qdrant_liveness_result(&span, config, &result);
    result
}

async fn check_qdrant_liveness_once(
    config: &QdrantLivenessConfig,
) -> Result<QdrantLivenessReport, QdrantLivenessError> {
    let api_key = load_qdrant_api_key(config)?;
    let client = build_qdrant_client(config, &api_key)?;

    let reply = tokio::time::timeout(config.timeout(), client.health_check())
        .await
        .map_err(|_elapsed| QdrantLivenessError::Timeout { timeout: config.timeout() })?
        .map_err(map_qdrant_error)?;

    tokio::time::timeout(config.timeout(), client.list_collections())
        .await
        .map_err(|_elapsed| QdrantLivenessError::Timeout { timeout: config.timeout() })?
        .map_err(map_qdrant_error)?;

    QdrantLivenessReport::try_from(reply)
}

fn read_api_key_file(path: &camino::Utf8Path) -> Result<String, QdrantLivenessError> {
    let root = Dir::open_ambient_dir("/", ambient_authority()).map_err(|source| {
        QdrantLivenessError::UnreadableApiKeyFile { path: path.to_path_buf(), source }
    })?;
    let relative_path = path.strip_prefix("/").unwrap_or(path);

    root.read_to_string(relative_path)
        .map_err(|source| map_api_key_read_error(path.to_path_buf(), source))
}

fn map_api_key_read_error(path: Utf8PathBuf, source: io::Error) -> QdrantLivenessError {
    if source.kind() == io::ErrorKind::NotFound {
        QdrantLivenessError::MissingApiKeyFile { path }
    } else {
        QdrantLivenessError::UnreadableApiKeyFile { path, source }
    }
}

fn build_qdrant_client(
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

fn is_authentication_failure_code(code: &str) -> bool {
    matches!(code, "Unauthenticated" | "PermissionDenied")
}

fn is_authentication_failure_status(code: &str, message: &str) -> bool {
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

#[cfg(test)]
mod tests;
