//! Opt-in live Qdrant integration tests for the liveness adapter.

use std::{
    future::Future,
    io::Write,
    process::{Command, Output},
    sync::atomic::{AtomicU64, Ordering},
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use qdrant_client::Qdrant;
use repovec_core::appliance::qdrant_liveness::{
    QdrantApiKey, QdrantLivenessConfig, QdrantLivenessError, QdrantLivenessReport,
    check_qdrant_liveness,
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
    let qdrant_container = QdrantContainer::start(container_name, TEST_API_KEY)
        .expect("Qdrant container should start");
    let config = config_for(&qdrant_container.endpoint, TEST_API_KEY, PROBE_TIMEOUT)
        .expect("integration config should build");
    block_on(wait_for_qdrant_liveness(&config, STARTUP_TIMEOUT))
        .expect("Tokio runtime should run the liveness wait")
        .expect("Qdrant should become live with the configured API key");
    let report = block_on(check_qdrant_liveness(&config))
        .expect("Tokio runtime should run the direct liveness check")
        .expect("direct liveness check should succeed after readiness");
    let collections = block_on(async {
        Qdrant::from_url(&qdrant_container.endpoint)
            .api_key(TEST_API_KEY)
            .skip_compatibility_check()
            .build()
            .map_err(|error| error.to_string())?
            .list_collections()
            .await
            .map_err(|error| error.to_string())
    })
    .expect("Tokio runtime should list Qdrant collections")
    .expect("configured API key should authenticate collection listing");

    assert!(collections.collections.is_empty());
    assert!(!report.version().is_empty());
}

#[test]
#[ignore = "requires Podman and network access"]
fn live_qdrant_rejects_a_wrong_api_key() {
    let container_name = container_name();
    let qdrant_container = QdrantContainer::start(container_name, TEST_API_KEY)
        .expect("Qdrant container should start");
    let ready_config = config_for(&qdrant_container.endpoint, TEST_API_KEY, PROBE_TIMEOUT)
        .expect("integration readiness config should build");
    block_on(wait_for_qdrant_liveness(&ready_config, STARTUP_TIMEOUT))
        .expect("Tokio runtime should run the readiness wait")
        .expect("Qdrant should become live before testing authentication failure");

    let wrong_config = config_for(&qdrant_container.endpoint, WRONG_API_KEY, PROBE_TIMEOUT)
        .expect("wrong-key integration config should build");
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
    let config = config_for("http://127.0.0.1:9", TEST_API_KEY, Duration::from_millis(500))
        .expect("closed-port integration config should build");
    let error = block_on(check_qdrant_liveness(&config))
        .expect("Tokio runtime should run the closed-port liveness check")
        .expect_err("closed port should fail liveness");

    assert!(matches!(error, QdrantLivenessError::GrpcUnavailable { .. }));
}

#[test]
#[ignore = "requires explicit live Qdrant integration run"]
fn waiting_for_qdrant_liveness_times_out() {
    let config = config_for("http://127.0.0.1:9", TEST_API_KEY, Duration::from_millis(50))
        .expect("timeout integration config should build");
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
    raw_api_key: &str,
    timeout: Duration,
) -> Result<QdrantLivenessConfig, String> {
    let api_key = QdrantApiKey::parse(raw_api_key)
        .map_err(|error| format!("integration API key should be valid: {error}"))?;

    Ok(QdrantLivenessConfig::new(endpoint, api_key, timeout))
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
    drop(stderr.write_fmt(arguments));
    drop(stderr.write_all(b"\n"));
}
