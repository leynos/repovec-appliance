//! Runtime liveness checks for the local Qdrant service.
//!
//! The static [`crate::appliance::qdrant_quadlet`] validator proves that the
//! checked-in Quadlet is wired correctly. This module proves the runtime half
//! of the same appliance contract: local Rust callers can load the stored API
//! key, connect to Qdrant's gRPC endpoint, and receive non-secret readiness
//! evidence from the service.

use std::{fmt, time::Duration};

use camino::Utf8PathBuf;

/// Qdrant's appliance gRPC endpoint.
pub const DEFAULT_QDRANT_GRPC_ENDPOINT: &str = "http://localhost:6334";

/// Location populated by the Qdrant API-key provisioning service.
pub const DEFAULT_QDRANT_API_KEY_PATH: &str = "/etc/repovec/qdrant-api-key";

/// Maximum time spent waiting for one Qdrant liveness probe by default.
pub const DEFAULT_QDRANT_LIVENESS_TIMEOUT: Duration = Duration::from_secs(5);

/// Configuration for the Qdrant runtime liveness probe.
///
/// # Examples
///
/// ```
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
    /// ```
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
/// ```
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
    /// ```
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

#[cfg(test)]
mod tests {
    //! Unit tests for Qdrant liveness domain values.

    use proptest::prelude::*;
    use rstest::rstest;

    use super::{QdrantApiKey, QdrantLivenessConfig, QdrantLivenessError, QdrantLivenessReport};

    #[rstest]
    #[case("0123456789abcdef")]
    #[case("repovec-qdrant-api-key")]
    #[case("abc DEF 123")]
    fn api_key_accepts_grpc_metadata_values(#[case] value: &str) {
        let key = QdrantApiKey::parse(value).expect("valid API key should parse");

        assert_eq!(key.as_secret(), value);
    }

    #[rstest]
    #[case("")]
    fn api_key_rejects_empty_values(#[case] value: &str) {
        assert!(matches!(QdrantApiKey::parse(value), Err(QdrantLivenessError::EmptyApiKey)));
    }

    #[rstest]
    #[case("abc\n")]
    #[case("abc\r")]
    #[case("abc\t")]
    #[case("abc\u{7f}")]
    #[case("abc\u{80}")]
    fn api_key_rejects_invalid_metadata_values(#[case] value: &str) {
        assert!(matches!(QdrantApiKey::parse(value), Err(QdrantLivenessError::InvalidApiKey)));
    }

    #[test]
    fn api_key_debug_is_redacted() {
        let key = QdrantApiKey::parse("super-secret").expect("valid API key should parse");

        assert_eq!(format!("{key:?}"), "QdrantApiKey(\"<redacted>\")");
    }

    #[test]
    fn invalid_api_key_display_is_redacted() {
        let error = QdrantApiKey::parse("super-secret\n")
            .expect_err("newline-suffixed API key should fail");

        assert!(!error.to_string().contains("super-secret"));
    }

    proptest! {
        #[test]
        fn api_key_accepts_non_empty_printable_ascii_values(value in "[\\x20-\\x7e]+") {
            prop_assert!(QdrantApiKey::parse(value).is_ok());
        }

        #[test]
        fn api_key_rejects_values_containing_non_printable_bytes(
            prefix in "[\\x20-\\x7e]*",
            invalid in prop_oneof![
                Just('\u{0}'),
                Just('\n'),
                Just('\r'),
                Just('\t'),
                Just('\u{7f}'),
                Just('é'),
            ],
            suffix in "[\\x20-\\x7e]*",
        ) {
            let value = format!("{prefix}{invalid}{suffix}");

            prop_assert!(QdrantApiKey::parse(value).is_err());
        }
    }

    #[test]
    fn liveness_report_exposes_server_metadata_with_commit() {
        let report = QdrantLivenessReport::new("qdrant", "1.15.0", Some("abc123"));

        assert_eq!(report.title(), "qdrant");
        assert_eq!(report.version(), "1.15.0");
        assert_eq!(report.commit(), Some("abc123"));
    }

    #[test]
    fn liveness_report_exposes_server_metadata_without_commit() {
        let report = QdrantLivenessReport::new("qdrant", "1.15.0", None::<String>);

        assert_eq!(report.title(), "qdrant");
        assert_eq!(report.version(), "1.15.0");
        assert_eq!(report.commit(), None);
    }

    #[test]
    fn default_config_matches_appliance_contract() {
        let config = QdrantLivenessConfig::default();

        assert_eq!(config.endpoint(), "http://localhost:6334");
        assert_eq!(config.api_key_path().as_str(), "/etc/repovec/qdrant-api-key");
    }
}
