//! Observer boundary for Qdrant Quadlet validation telemetry.
//!
//! The validation pipeline reports operator-facing events through
//! [`QdrantQuadletObserver`] so telemetry side effects are explicit in function
//! signatures. [`TracingQdrantQuadletObserver`] is the production adapter that
//! maps those events to `tracing` records under the shared module target.

use tracing::{error, info, warn};

use super::LOG_TARGET;

/// Receives Qdrant Quadlet validation telemetry.
///
/// Implementations can forward events to tracing, capture them in tests, or
/// ignore them. Default method bodies are no-ops so callers can pass `()` when
/// they need validation without telemetry.
///
/// Re-use plan: keep this trait as the narrow observer port for the current
/// Qdrant Quadlet validator while [issue #36] tracks whether it should be
/// collapsed into a smaller shared validation observer. New appliance
/// validators should not copy these 22 Qdrant-specific callbacks; instead,
/// define domain-specific callbacks only when they need Qdrant fields, or wait
/// for the shared observer shape from issue #36 when their events are generic.
///
/// [issue #36]: https://github.com/leynos/repovec-appliance/issues/36
pub trait QdrantQuadletObserver {
    /// Records validation of the checked-in Quadlet asset.
    fn validating_checked_in_qdrant_quadlet(&self, _path: &str) {}

    /// Records entry into arbitrary Quadlet contract validation.
    fn validating_qdrant_quadlet_contract(&self) {}

    /// Records successful completion of Quadlet contract validation.
    fn qdrant_quadlet_contract_validation_succeeded(&self) {}

    /// Records a malformed line lacking an `=` separator.
    fn invalid_line(&self, _line_number: usize, _redacted_line: &str) {}

    /// Records a property appearing before any section header.
    fn property_before_section(&self, _line_number: usize, _redacted_line: &str) {}

    /// Records a missing `Image=` directive.
    fn missing_image(&self, _expected_image: &str) {}

    /// Records an image reference that is not fully qualified and pinned.
    fn image_not_fully_qualified(&self, _image: &str, _expected_image: &str) {}

    /// Records an unexpected or duplicate image value.
    fn unexpected_image(&self, _image: &str, _expected_image: &str) {}

    /// Records a missing `PublishPort=` mapping.
    fn missing_publish_port(&self, _port: u16, _expected_publish_port: &str) {}

    /// Records a `PublishPort=` mapping that is not loopback-bound.
    fn publish_port_not_bound_to_loopback(
        &self,
        _port: u16,
        _publish_port: &str,
        _expected_publish_port: &str,
    ) {
    }

    /// Records a missing storage mount candidate.
    fn missing_storage_mount(&self, _expected_source: &str, _expected_target: &str) {}

    /// Records a malformed storage mount candidate.
    fn missing_storage_mount_volume(
        &self,
        _volume: &str,
        _expected_source: &str,
        _expected_target: &str,
    ) {
    }

    /// Records an incorrect storage source.
    fn incorrect_storage_source(&self, _source: &str, _expected_source: &str) {}

    /// Records an incorrect storage target.
    fn incorrect_storage_target(&self, _storage_target: &str, _expected_target: &str) {}

    /// Records a missing `SELinux` relabel option.
    fn missing_selinux_relabel(&self, _volume: &str, _expected_selinux_relabel: &str) {}

    /// Records a missing auto-update policy.
    fn missing_auto_update(&self, _expected_auto_update: &str) {}

    /// Records an incorrect or duplicate auto-update policy.
    fn incorrect_auto_update(&self, _auto_update: &str, _expected_auto_update: &str) {}

    /// Records a missing API-key provisioning dependency.
    fn missing_api_key_provisioning_dependency(
        &self,
        _directive: &str,
        _expected_dependency: &str,
    ) {
    }

    /// Records an incorrect API-key provisioning dependency.
    fn incorrect_api_key_provisioning_dependency(
        &self,
        _directive: &str,
        _dependency: &str,
        _expected_dependency: &str,
    ) {
    }

    /// Records a missing API-key secret entry.
    fn missing_api_key_secret(&self, _expected_secret: &str, _expected_target: &str) {}

    /// Records an incorrect API-key secret entry.
    fn incorrect_api_key_secret(
        &self,
        _secret: &str,
        _expected_secret: &str,
        _expected_target: &str,
    ) {
    }

    /// Records an inline API-key environment assignment.
    fn inline_api_key_environment(
        &self,
        _environment: &str,
        _expected_secret: &str,
        _expected_target: &str,
    ) {
    }
}

impl QdrantQuadletObserver for () {}

/// Emits Qdrant Quadlet validation telemetry through `tracing`.
#[derive(Clone, Copy, Debug, Default)]
pub struct TracingQdrantQuadletObserver;

impl QdrantQuadletObserver for TracingQdrantQuadletObserver {
    fn validating_checked_in_qdrant_quadlet(&self, path: &str) {
        info!(target: LOG_TARGET, path, "validating checked-in qdrant quadlet");
    }

    fn validating_qdrant_quadlet_contract(&self) {
        info!(target: LOG_TARGET, "validating qdrant quadlet contract");
    }

    fn qdrant_quadlet_contract_validation_succeeded(&self) {
        info!(target: LOG_TARGET, "qdrant quadlet contract validation succeeded");
    }

    fn invalid_line(&self, line_number: usize, redacted_line: &str) {
        error!(
            target: LOG_TARGET,
            line_number,
            redacted_line,
            "qdrant quadlet validation rejected invalid line"
        );
    }

    fn property_before_section(&self, line_number: usize, redacted_line: &str) {
        error!(
            target: LOG_TARGET,
            line_number,
            redacted_line,
            "qdrant quadlet validation rejected property before section"
        );
    }

    fn missing_image(&self, expected_image: &str) {
        warn!(
            target: LOG_TARGET,
            expected_image,
            "qdrant quadlet validation failed: missing image"
        );
    }

    fn image_not_fully_qualified(&self, image: &str, expected_image: &str) {
        warn!(
            target: LOG_TARGET,
            image,
            expected_image,
            "qdrant quadlet validation failed: image is not fully qualified and pinned"
        );
    }

    fn unexpected_image(&self, image: &str, expected_image: &str) {
        warn!(
            target: LOG_TARGET,
            image,
            expected_image,
            "qdrant quadlet validation failed: unexpected image"
        );
    }

    fn missing_publish_port(&self, port: u16, expected_publish_port: &str) {
        warn!(
            target: LOG_TARGET,
            port,
            expected_publish_port,
            "qdrant quadlet validation failed: missing publish port"
        );
    }

    fn publish_port_not_bound_to_loopback(
        &self,
        port: u16,
        publish_port: &str,
        expected_publish_port: &str,
    ) {
        warn!(
            target: LOG_TARGET,
            port,
            publish_port,
            expected_publish_port,
            "qdrant quadlet validation failed: publish port is not bound to loopback"
        );
    }

    fn missing_storage_mount(&self, expected_source: &str, expected_target: &str) {
        warn!(
            target: LOG_TARGET,
            expected_source,
            expected_target,
            "qdrant quadlet validation failed: missing storage mount"
        );
    }

    fn missing_storage_mount_volume(
        &self,
        volume: &str,
        expected_source: &str,
        expected_target: &str,
    ) {
        warn!(
            target: LOG_TARGET,
            volume,
            expected_source,
            expected_target,
            "qdrant quadlet validation failed: missing storage mount"
        );
    }

    fn incorrect_storage_source(&self, source: &str, expected_source: &str) {
        warn!(
            target: LOG_TARGET,
            source,
            expected_source,
            "qdrant quadlet validation failed: incorrect storage source"
        );
    }

    fn incorrect_storage_target(&self, storage_target: &str, expected_target: &str) {
        warn!(
            target: LOG_TARGET,
            storage_target,
            expected_target,
            "qdrant quadlet validation failed: incorrect storage target"
        );
    }

    fn missing_selinux_relabel(&self, volume: &str, expected_selinux_relabel: &str) {
        warn!(
            target: LOG_TARGET,
            volume,
            expected_selinux_relabel,
            "qdrant quadlet validation failed: missing selinux relabel"
        );
    }

    fn missing_auto_update(&self, expected_auto_update: &str) {
        warn!(
            target: LOG_TARGET,
            expected_auto_update,
            "qdrant quadlet validation failed: missing auto-update policy"
        );
    }

    fn incorrect_auto_update(&self, auto_update: &str, expected_auto_update: &str) {
        warn!(
            target: LOG_TARGET,
            auto_update,
            expected_auto_update,
            "qdrant quadlet validation failed: incorrect auto-update policy"
        );
    }

    fn missing_api_key_provisioning_dependency(&self, directive: &str, expected_dependency: &str) {
        warn!(
            target: LOG_TARGET,
            directive,
            expected_dependency,
            "qdrant quadlet validation failed: missing api key provisioning dependency"
        );
    }

    fn incorrect_api_key_provisioning_dependency(
        &self,
        directive: &str,
        dependency: &str,
        expected_dependency: &str,
    ) {
        warn!(
            target: LOG_TARGET,
            directive,
            dependency,
            expected_dependency,
            "qdrant quadlet validation failed: incorrect api key provisioning dependency"
        );
    }

    fn missing_api_key_secret(&self, expected_secret: &str, expected_target: &str) {
        warn!(
            target: LOG_TARGET,
            expected_secret,
            expected_target,
            "qdrant quadlet validation failed: missing api key secret"
        );
    }

    fn incorrect_api_key_secret(&self, secret: &str, expected_secret: &str, expected_target: &str) {
        warn!(
            target: LOG_TARGET,
            secret,
            expected_secret,
            expected_target,
            "qdrant quadlet validation failed: incorrect api key secret"
        );
    }

    fn inline_api_key_environment(
        &self,
        environment: &str,
        expected_secret: &str,
        expected_target: &str,
    ) {
        warn!(
            target: LOG_TARGET,
            environment,
            expected_secret,
            expected_target,
            "qdrant quadlet validation failed: inline api key environment is disallowed"
        );
    }
}
