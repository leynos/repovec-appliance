//! Unit tests for Qdrant liveness domain values and file adapters.

use std::{
    sync::atomic::{AtomicU64, Ordering},
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use camino::Utf8PathBuf;
use cap_std::{ambient_authority, fs_utf8::Dir};
use proptest::prelude::*;
use qdrant_client::qdrant::HealthCheckReply;
use rstest::rstest;

use super::{
    DEFAULT_QDRANT_LIVENESS_TIMEOUT, QdrantApiKey, QdrantLivenessConfig, QdrantLivenessError,
    QdrantLivenessReport,
    adapter::{
        build_qdrant_client, is_authentication_failure_code, is_authentication_failure_status,
    },
    load_qdrant_api_key,
};

const TEST_QDRANT_ENDPOINT: &str = "http://127.0.0.1:6334";
const TEST_API_KEY: &str = "repovec-test-qdrant-api-key";

static TEMP_COUNTER: AtomicU64 = AtomicU64::new(0);

struct TempKeyFile {
    path: Utf8PathBuf,
    dir_name: String,
}

impl TempKeyFile {
    fn create(contents: &str) -> Result<Self, std::io::Error> {
        let temp_root = temp_root();
        let root = Dir::open_ambient_dir(&temp_root, ambient_authority())?;
        let dir_name = unique_temp_dir_name();
        root.create_dir(&dir_name)?;
        root.write(format!("{dir_name}/qdrant-api-key"), contents)?;

        Ok(Self { path: temp_root.join(&dir_name).join("qdrant-api-key"), dir_name })
    }
}

impl Drop for TempKeyFile {
    fn drop(&mut self) {
        let temp_root = temp_root();
        let Ok(root) = Dir::open_ambient_dir(&temp_root, ambient_authority()) else {
            return;
        };
        if root.remove_dir_all(&self.dir_name).is_err() {}
    }
}

fn temp_root() -> Utf8PathBuf { Utf8PathBuf::from("/tmp") }

fn unique_temp_dir_name() -> String {
    let count = TEMP_COUNTER.fetch_add(1, Ordering::Relaxed);
    let elapsed = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or(Duration::ZERO);

    format!("repovec-qdrant-liveness-{}-{}-{count}", std::process::id(), elapsed.as_nanos())
}

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
    let error =
        QdrantApiKey::parse("super-secret\n").expect_err("newline-suffixed API key should fail");

    assert!(!error.to_string().contains("super-secret"));
}

#[test]
fn load_qdrant_api_key_reads_configured_key_file() {
    let key_file =
        TempKeyFile::create("0123456789abcdef").expect("temporary key file should be created");
    let key = load_qdrant_api_key(&key_file.path)
        .expect("configured API-key file should load successfully");

    assert_eq!(key.as_secret(), "0123456789abcdef");
}

#[test]
fn load_qdrant_api_key_maps_missing_file() {
    let path = temp_root().join(unique_temp_dir_name()).join("missing-key");
    let error = load_qdrant_api_key(&path).expect_err("missing key file should fail");

    assert!(
        matches!(error, QdrantLivenessError::MissingApiKeyFile { path: error_path } if error_path == path)
    );
}

#[rstest]
#[case("", QdrantLivenessError::EmptyApiKey)]
#[case("0123456789abcdef\n", QdrantLivenessError::InvalidApiKey)]
fn load_qdrant_api_key_rejects_invalid_file_contents(
    #[case] contents: &str,
    #[case] expected_error: QdrantLivenessError,
) {
    let key_file = TempKeyFile::create(contents).expect("temporary key file should be created");
    let error = load_qdrant_api_key(&key_file.path).expect_err("invalid API-key file should fail");

    assert_eq!(error.to_string(), expected_error.to_string());
    assert!(!error.to_string().contains("0123456789abcdef"));
}

#[test]
fn load_qdrant_api_key_maps_unreadable_paths() {
    let error = load_qdrant_api_key(temp_root().as_path())
        .expect_err("directory path should be unreadable");

    assert!(matches!(error, QdrantLivenessError::UnreadableApiKeyFile { .. }));
}

#[test]
fn build_qdrant_client_maps_invalid_endpoint() {
    let key = QdrantApiKey::parse("0123456789abcdef").expect("valid API key should parse");
    let config = QdrantLivenessConfig::new("not a uri", key, Duration::from_secs(1));

    let Err(error) = build_qdrant_client(&config, config.api_key()) else {
        panic!("invalid endpoint should fail");
    };

    assert!(
        matches!(error, QdrantLivenessError::InvalidEndpoint { endpoint } if endpoint == "not a uri")
    );
}

#[rstest]
#[case("Unauthenticated", true)]
#[case("PermissionDenied", true)]
#[case("Unavailable", false)]
#[case("DeadlineExceeded", false)]
fn authentication_failure_codes_are_distinct(#[case] code: &str, #[case] expected: bool) {
    assert_eq!(is_authentication_failure_code(code), expected);
}

#[rstest]
#[case(
    "Unknown",
    "The request does not have valid authentication credentials: Invalid API key or JWT"
)]
#[case("Unknown", "Invalid API key or JWT")]
#[case("Unknown", "Invalid JWT")]
fn qdrant_authentication_messages_are_classified(#[case] code: &str, #[case] message: &str) {
    assert!(is_authentication_failure_status(code, message));
}

#[test]
fn liveness_report_accepts_health_reply_with_version() {
    let reply = HealthCheckReply {
        title: String::from("qdrant"),
        version: String::from("1.15.0"),
        commit: Some(String::from("abc123")),
    };

    let report =
        QdrantLivenessReport::try_from(reply).expect("versioned health reply should convert");

    assert_eq!(report.title(), "qdrant");
    assert_eq!(report.version(), "1.15.0");
    assert_eq!(report.commit(), Some("abc123"));
}

#[test]
fn liveness_report_rejects_health_reply_without_version() {
    let reply =
        HealthCheckReply { title: String::from("qdrant"), version: String::new(), commit: None };

    let error = QdrantLivenessReport::try_from(reply)
        .expect_err("health reply without version should fail");

    assert!(matches!(error, QdrantLivenessError::MissingServerVersion));
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
fn in_memory_config_retains_endpoint_and_timeout_contract() {
    let config = QdrantLivenessConfig::new(
        TEST_QDRANT_ENDPOINT,
        test_api_key().expect("test API key should be valid"),
        DEFAULT_QDRANT_LIVENESS_TIMEOUT,
    );

    assert_eq!(config.endpoint(), TEST_QDRANT_ENDPOINT);
    assert_eq!(config.timeout(), DEFAULT_QDRANT_LIVENESS_TIMEOUT);
}

#[test]
fn liveness_success_observability_uses_a_bounded_metric() -> Result<(), String> {
    let config = QdrantLivenessConfig::new(
        TEST_QDRANT_ENDPOINT,
        test_api_key().map_err(|error| error.to_string())?,
        Duration::from_millis(7),
    );
    let result = Ok(QdrantLivenessReport::new("qdrant", "1.15.0", None::<String>));
    let ((), logs) = repovec_test_helpers::capture_logs(|| {
        let span = super::observability::qdrant_liveness_span(&config);
        super::observability::record_qdrant_liveness_result(&span, &config, &result);
    })?;

    repovec_test_helpers::ensure_log_line_contains(
        &logs,
        "DEBUG",
        "endpoint=\"http://127.0.0.1:6334\"",
        "success log should include the Qdrant endpoint",
    )?;
    repovec_test_helpers::ensure_log_line_contains(
        &logs,
        "INFO",
        "metric.qdrant_liveness_success_total",
        "success should emit a metric event",
    )
}
#[test]
fn liveness_failure_observability_includes_probe_context() -> Result<(), String> {
    let config = QdrantLivenessConfig::new(
        TEST_QDRANT_ENDPOINT,
        test_api_key().map_err(|error| error.to_string())?,
        Duration::from_millis(7),
    );
    let result = Err(QdrantLivenessError::MissingApiKeyFile {
        path: Utf8PathBuf::from("/etc/repovec/qdrant-api-key"),
    });

    let ((), logs) = repovec_test_helpers::capture_logs(|| {
        let span = super::observability::qdrant_liveness_span(&config);
        super::observability::record_qdrant_liveness_result(&span, &config, &result);
    })?;

    repovec_test_helpers::ensure_log_line_contains(
        &logs,
        "DEBUG",
        "endpoint=\"http://127.0.0.1:6334\"",
        "failure log should include the Qdrant endpoint",
    )?;
    repovec_test_helpers::ensure_log_line_contains(
        &logs,
        "DEBUG",
        "timeout_ms=7",
        "failure log should include the probe timeout",
    )?;
    repovec_test_helpers::ensure_log_line_contains(
        &logs,
        "DEBUG",
        "error_category=\"missing_api_key_file\"",
        "failure log should include the error category",
    )?;
    repovec_test_helpers::ensure_log_line_contains(
        &logs,
        "INFO",
        "metric.qdrant_liveness_failure_total",
        "failure should emit a bounded metric event",
    )
}

fn test_api_key() -> Result<QdrantApiKey, QdrantLivenessError> { QdrantApiKey::parse(TEST_API_KEY) }

#[test]
fn qdrant_liveness_error_display_matches_contract() {
    let rendered = [
        QdrantLivenessError::MissingApiKeyFile {
            path: Utf8PathBuf::from("/etc/repovec/qdrant-api-key"),
        },
        QdrantLivenessError::UnreadableApiKeyFile {
            path: Utf8PathBuf::from("/etc/repovec/qdrant-api-key"),
            source: std::io::Error::new(std::io::ErrorKind::PermissionDenied, "permission denied"),
        },
        QdrantLivenessError::EmptyApiKey,
        QdrantLivenessError::InvalidApiKey,
        QdrantLivenessError::InvalidEndpoint { endpoint: String::from("not a uri") },
        QdrantLivenessError::Timeout { timeout: Duration::from_millis(250) },
        QdrantLivenessError::AuthenticationFailed,
        QdrantLivenessError::GrpcUnavailable {
            message: String::from("transport error: connection refused"),
        },
        QdrantLivenessError::MissingServerVersion,
    ]
    .into_iter()
    .map(|error| error.to_string())
    .collect::<Vec<_>>()
    .join("\n");

    insta::assert_snapshot!(
        &rendered,
        @r"
Qdrant API-key file is missing: /etc/repovec/qdrant-api-key
Qdrant API-key file cannot be read: /etc/repovec/qdrant-api-key
Qdrant API key is empty
Qdrant API key is not a valid gRPC metadata value: <redacted>
Qdrant gRPC endpoint is invalid: not a uri
Qdrant liveness check timed out after 250ms
Qdrant rejected the configured API key
Qdrant gRPC liveness check failed: transport error: connection refused
Qdrant health reply did not include a server version"
    );
}
