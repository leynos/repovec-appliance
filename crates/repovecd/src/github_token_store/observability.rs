//! Tracing helpers for encrypted GitHub token storage.

use std::time::Instant;

use tracing::info_span;

pub(super) fn info_token_store_write(started_at: Instant) {
    let duration_span = info_span!(
        "metric.github_token_store_write_duration_ms",
        elapsed_ms = started_at.elapsed().as_millis(),
    );
    let _duration_entered = duration_span.enter();
    let total_span = info_span!("metric.github_token_store_write_total");
    let _total_entered = total_span.enter();
}

pub(super) fn info_token_store_load(started_at: Instant) {
    let duration_span = info_span!(
        "metric.github_token_store_load_duration_ms",
        elapsed_ms = started_at.elapsed().as_millis(),
    );
    let _duration_entered = duration_span.enter();
    let total_span = info_span!("metric.github_token_store_load_total");
    let _total_entered = total_span.enter();
}
