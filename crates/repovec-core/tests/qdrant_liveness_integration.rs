//! Opt-in live Qdrant integration tests for the liveness adapter.

use std::{
    future::Future,
    io::Write,
    process::{Command, Output},
    sync::atomic::{AtomicU64, Ordering},
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use camino::Utf8PathBuf;
use cap_std::{ambient_authority, fs_utf8::Dir};
use repovec_core::appliance::qdrant_liveness::{
    QdrantLivenessConfig, QdrantLivenessError, QdrantLivenessReport, check_qdrant_liveness,
};

const QDRANT_IMAGE: &str = "docker.io/qdrant/qdrant:v1.18.1";
const QDRANT_GRPC_PORT: &str = "6334/tcp";
const TEST_API_KEY: &str = "repovec-integration-test-api-key";
const WRONG_API_KEY: &str = "repovec-integration-test-wrong-key";
const STARTUP_TIMEOUT: Duration = Duration::from_secs(45);
const PROBE_TIMEOUT: Duration = Duration::from_secs(2);
const POLL_INTERVAL: Duration = Duration::from_millis(250);

static TEMP_COUNTER: AtomicU64 = AtomicU64::new(0);

#[test]
#[ignore = "requires Podman and network access"]
fn live_qdrant_accepts_the_configured_api_key() {
    let container_name = container_name();
    let api_key_file =
        TemporaryApiKeyFile::create(TEST_API_KEY).expect("API-key file should be created");
    let qdrant_container = QdrantContainer::start(container_name, TEST_API_KEY)
        .expect("Qdrant container should start");
    let config = config_for(&qdrant_container.endpoint, &api_key_file.path, PROBE_TIMEOUT);
    let report = block_on(wait_for_qdrant_liveness(&config, STARTUP_TIMEOUT))
        .expect("Tokio runtime should run the liveness wait")
        .expect("Qdrant should become live with the configured API key");

    assert!(!report.version().is_empty());
}

#[test]
#[ignore = "requires Podman and network access"]
fn live_qdrant_rejects_a_wrong_api_key() {
    let container_name = container_name();
    let wrong_api_key_file =
        TemporaryApiKeyFile::create(WRONG_API_KEY).expect("wrong API-key file should be created");
    let qdrant_container = QdrantContainer::start(container_name, TEST_API_KEY)
        .expect("Qdrant container should start");
    let correct_api_key_file =
        TemporaryApiKeyFile::create(TEST_API_KEY).expect("correct API-key file should be created");
    let ready_config =
        config_for(&qdrant_container.endpoint, &correct_api_key_file.path, PROBE_TIMEOUT);
    block_on(wait_for_qdrant_liveness(&ready_config, STARTUP_TIMEOUT))
        .expect("Tokio runtime should run the readiness wait")
        .expect("Qdrant should become live before testing authentication failure");

    let wrong_config =
        config_for(&qdrant_container.endpoint, &wrong_api_key_file.path, PROBE_TIMEOUT);
    let error = block_on(check_qdrant_liveness(&wrong_config))
        .expect("Tokio runtime should run the wrong-key liveness check")
        .expect_err("wrong API key should be rejected");

    assert!(
        matches!(error, QdrantLivenessError::AuthenticationFailed),
        "wrong key should map to AuthenticationFailed, got {error:?}"
    );
}

#[test]
#[ignore = "requires explicit live Qdrant integration run"]
fn liveness_fails_when_the_service_port_is_closed() {
    let api_key_file =
        TemporaryApiKeyFile::create(TEST_API_KEY).expect("API-key file should be created");
    let config = config_for("http://127.0.0.1:9", &api_key_file.path, Duration::from_millis(500));
    let error = block_on(check_qdrant_liveness(&config))
        .expect("Tokio runtime should run the closed-port liveness check")
        .expect_err("closed port should fail liveness");

    assert!(matches!(error, QdrantLivenessError::GrpcUnavailable { .. }));
}

#[test]
#[ignore = "requires explicit live Qdrant integration run"]
fn waiting_for_qdrant_liveness_times_out() {
    let api_key_file =
        TemporaryApiKeyFile::create(TEST_API_KEY).expect("API-key file should be created");
    let config = config_for("http://127.0.0.1:9", &api_key_file.path, Duration::from_millis(50));
    let error = block_on(wait_for_qdrant_liveness(&config, Duration::from_millis(75)))
        .expect("Tokio runtime should run the readiness timeout check")
        .expect_err("readiness wait should time out");

    assert!(matches!(error, QdrantLivenessError::Timeout { .. }));
}

async fn wait_for_qdrant_liveness(
    config: &QdrantLivenessConfig,
    timeout: Duration,
) -> Result<QdrantLivenessReport, QdrantLivenessError> {
    let deadline = Instant::now() + timeout;

    loop {
        let liveness_result = check_qdrant_liveness(config).await;
        let now = Instant::now();

        match liveness_result {
            Ok(report) => return Ok(report),
            Err(_) if now >= deadline => return Err(wait_timeout_error(timeout)),
            Err(_) => {
                let remaining = deadline.saturating_duration_since(now);
                tokio::time::sleep(POLL_INTERVAL.min(remaining)).await;
            }
        }
    }
}

const fn wait_timeout_error(timeout: Duration) -> QdrantLivenessError {
    QdrantLivenessError::Timeout { timeout }
}

fn config_for(
    endpoint: &str,
    api_key_path: &camino::Utf8Path,
    timeout: Duration,
) -> QdrantLivenessConfig {
    QdrantLivenessConfig::new(endpoint, api_key_path.to_path_buf(), timeout)
}

fn container_name() -> String { format!("repovec-qdrant-liveness-{}", unique_suffix()) }

fn block_on<T>(future: impl Future<Output = T>) -> Result<T, String> {
    let runtime =
        match tokio::runtime::Builder::new_current_thread().enable_io().enable_time().build() {
            Ok(runtime) => runtime,
            Err(error) => {
                return Err(format!("Tokio runtime should build for integration tests: {error}"));
            }
        };

    Ok(runtime.block_on(future))
}

fn unique_suffix() -> String {
    let counter = TEMP_COUNTER.fetch_add(1, Ordering::Relaxed);
    let nanos = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or(Duration::ZERO).as_nanos();

    format!("{nanos}-{counter}")
}

struct QdrantContainer {
    name: String,
    endpoint: String,
}

impl QdrantContainer {
    fn start(name: String, api_key: &str) -> Result<Self, String> {
        let output = podman([
            "run",
            "--detach",
            "--rm",
            "--name",
            &name,
            "--env",
            &format!("QDRANT__SERVICE__API_KEY={api_key}"),
            "--publish",
            "127.0.0.1::6334",
            QDRANT_IMAGE,
        ])?;
        assert_success("podman run", &output)?;

        let endpoint = match published_grpc_endpoint(&name) {
            Ok(endpoint) => endpoint,
            Err(error) => {
                log_cleanup_result(
                    "podman rm --force",
                    &name,
                    Command::new("podman").args(["rm", "--force", &name]).output(),
                );
                return Err(error);
            }
        };
        Ok(Self { name, endpoint })
    }
}

impl Drop for QdrantContainer {
    fn drop(&mut self) {
        log_cleanup_result(
            "podman rm --force",
            &self.name,
            Command::new("podman").args(["rm", "--force", &self.name]).output(),
        );
    }
}

fn published_grpc_endpoint(container_name: &str) -> Result<String, String> {
    let output = podman(["port", container_name, QDRANT_GRPC_PORT])?;
    assert_success("podman port", &output)?;
    let stdout = String::from_utf8_lossy(&output.stdout);
    let Some(endpoint) = stdout.lines().map(str::trim).find(|line| !line.is_empty()) else {
        return Err(format!("podman port returned no {QDRANT_GRPC_PORT} binding"));
    };

    Ok(format!("http://{endpoint}"))
}

fn podman<const N: usize>(args: [&str; N]) -> Result<Output, String> {
    match Command::new("podman").args(args).output() {
        Ok(output) => Ok(output),
        Err(error) => Err(format!("podman command should start: {error}")),
    }
}

fn assert_success(command: &str, output: &Output) -> Result<(), String> {
    if output.status.success() {
        Ok(())
    } else {
        Err(format!(
            "{command} failed with status {:?}: {}",
            output.status.code(),
            String::from_utf8_lossy(&output.stderr)
        ))
    }
}

struct TemporaryApiKeyFile {
    directory: Utf8PathBuf,
    filename: String,
    path: Utf8PathBuf,
}

impl TemporaryApiKeyFile {
    fn create(api_key: &str) -> Result<Self, String> {
        let directory = temp_directory()?;
        let filename = format!("repovec-qdrant-api-key-{}", unique_suffix());
        let path = directory.join(&filename);
        let dir = match Dir::open_ambient_dir(&directory, ambient_authority()) {
            Ok(dir) => dir,
            Err(error) => return Err(format!("temporary directory should open: {error}")),
        };

        if let Err(error) = dir.write(&filename, api_key) {
            return Err(format!("temporary API-key file should be written: {error}"));
        }

        Ok(Self { directory, filename, path })
    }
}

impl Drop for TemporaryApiKeyFile {
    fn drop(&mut self) {
        match Dir::open_ambient_dir(&self.directory, ambient_authority()) {
            Ok(dir) => {
                if let Err(error) = dir.remove_file(&self.filename) {
                    write_cleanup_warning(format_args!(
                        "failed to remove temporary Qdrant API-key file {}: {error}",
                        self.path
                    ));
                }
            }
            Err(error) => write_cleanup_warning(format_args!(
                "failed to open temporary directory {} for cleanup of {}: {error}",
                self.directory, self.filename
            )),
        }
    }
}

fn temp_directory() -> Result<Utf8PathBuf, String> {
    Utf8PathBuf::from_path_buf(std::env::temp_dir())
        .map_err(|path| format!("temporary directory path is not UTF-8: {}", path.display()))
}

fn log_cleanup_result(command: &str, subject: &str, cleanup_result: std::io::Result<Output>) {
    match cleanup_result {
        Ok(command_output) if command_output.status.success() => {}
        Ok(command_output) => write_cleanup_warning(format_args!(
            "{command} cleanup failed for {subject} with status {:?}; stdout: {}; stderr: {}",
            command_output.status.code(),
            String::from_utf8_lossy(&command_output.stdout),
            String::from_utf8_lossy(&command_output.stderr)
        )),
        Err(error) => write_cleanup_warning(format_args!(
            "{command} cleanup failed to start for {subject}: {error}"
        )),
    }
}

fn write_cleanup_warning(arguments: std::fmt::Arguments<'_>) {
    let mut stderr = std::io::stderr().lock();
    stderr.write_fmt(arguments).ok();
    stderr.write_all(b"\n").ok();
}
