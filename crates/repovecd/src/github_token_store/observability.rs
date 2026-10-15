//! Tracing helpers for encrypted GitHub token storage.

use std::time::Instant;

use tracing::info;

/// Records the duration and count of a completed token write, without token bytes.
/// For example, a completed write emits one duration and one total event.
pub(super) fn info_token_store_write(started_at: Instant) {
    info_duration("metric.github_token_store_write_duration_ms", started_at);
    info_total("metric.github_token_store_write_total");
}

/// Records the duration and count of a completed token load, without token bytes.
/// For example, a completed load emits one duration and one total event.
pub(super) fn info_token_store_load(started_at: Instant) {
    info_duration("metric.github_token_store_load_duration_ms", started_at);
    info_total("metric.github_token_store_load_total");
}

/// Emits elapsed milliseconds for the supplied metric name.
/// For example, a start instant ten milliseconds ago reports an elapsed value near ten.
fn info_duration(metric: &'static str, started_at: Instant) {
    info!("{metric} elapsed_ms={}", started_at.elapsed().as_millis());
}

/// Emits one event naming the supplied total metric.
/// For example, a write-total metric name records one completed write.
fn info_total(metric: &'static str) {
    info!("{metric}");
}
