//! Tracing helpers for encrypted GitHub token storage.

use std::time::Instant;

use tracing::info;

pub(super) fn info_token_store_write(started_at: Instant) {
    info_duration("metric.github_token_store_write_duration_ms", started_at);
    info_total("metric.github_token_store_write_total");
}

pub(super) fn info_token_store_load(started_at: Instant) {
    info_duration("metric.github_token_store_load_duration_ms", started_at);
    info_total("metric.github_token_store_load_total");
}

fn info_duration(metric: &'static str, started_at: Instant) {
    info!("{metric} elapsed_ms={}", started_at.elapsed().as_millis());
}

fn info_total(metric: &'static str) {
    info!("{metric}");
}
