//! Runtime liveness checks for the local Qdrant service.
//!
//! The static [`crate::appliance::qdrant_quadlet`] validator proves that the
//! checked-in Quadlet is wired correctly. This module proves the runtime half
//! of the same appliance contract: local Rust callers can load the stored API
//! key, connect to Qdrant's gRPC endpoint, and receive non-secret readiness
//! evidence from the service.

use std::{fmt, time::Duration};

use camino::Utf8PathBuf;

mod adapter;
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
    let contents = adapter::read_api_key_file(config.api_key_path())?;
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
    let result = adapter::check_qdrant_liveness_once(config).await;
    record_qdrant_liveness_result(&span, config, &result);
    result
}

#[cfg(test)]
mod tests;
