//! Unit tests for Qdrant liveness domain values and file adapters.

use std::{
    sync::atomic::{AtomicU64, Ordering},
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use camino::Utf8PathBuf;
use cap_std::{ambient_authority, fs_utf8::Dir};
use proptest::prelude::*;
use rstest::rstest;

use super::{
    QdrantApiKey, QdrantLivenessConfig, QdrantLivenessError, QdrantLivenessReport,
    load_qdrant_api_key,
};

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
    let config = QdrantLivenessConfig::new(
        "http://localhost:6334",
        key_file.path.clone(),
        Duration::from_secs(1),
    );

    let key =
        load_qdrant_api_key(&config).expect("configured API-key file should load successfully");

    assert_eq!(key.as_secret(), "0123456789abcdef");
}

#[test]
fn load_qdrant_api_key_maps_missing_file() {
    let path = temp_root().join(unique_temp_dir_name()).join("missing-key");
    let config =
        QdrantLivenessConfig::new("http://localhost:6334", path.clone(), Duration::from_secs(1));

    let error = load_qdrant_api_key(&config).expect_err("missing key file should fail");

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
    let config = QdrantLivenessConfig::new(
        "http://localhost:6334",
        key_file.path.clone(),
        Duration::from_secs(1),
    );

    let error = load_qdrant_api_key(&config).expect_err("invalid API-key file should fail");

    assert_eq!(error.to_string(), expected_error.to_string());
    assert!(!error.to_string().contains("0123456789abcdef"));
}

#[test]
fn load_qdrant_api_key_maps_unreadable_paths() {
    let config =
        QdrantLivenessConfig::new("http://localhost:6334", temp_root(), Duration::from_secs(1));

    let error = load_qdrant_api_key(&config).expect_err("directory path should be unreadable");

    assert!(matches!(error, QdrantLivenessError::UnreadableApiKeyFile { .. }));
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
