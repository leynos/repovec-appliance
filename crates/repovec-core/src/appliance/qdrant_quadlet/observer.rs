//! Observer boundary for Qdrant Quadlet validation telemetry.
//!
//! The validation pipeline reports operator-facing events through
//! [`QdrantQuadletObserver`] so telemetry side effects are explicit in function
//! signatures. [`TracingQdrantQuadletObserver`] is the production adapter that
//! maps those events to `tracing` records under the shared module target.

use super::QDRANT_API_KEY_SERVICE;

/// Receives Qdrant Quadlet validation telemetry.
///
/// Implementations can forward events to tracing, capture them in tests, or
/// ignore them. Observers must be thread-safe so validators can cross worker
/// boundaries without weakening the telemetry contract. Default method bodies
/// are no-ops so callers can pass `()` when they need validation without
/// telemetry.
///
/// Re-use plan: keep this trait as the narrow observer port for the current
/// Qdrant Quadlet validator while [issue #36] tracks whether it should be
/// collapsed into a smaller shared validation observer. New appliance
/// validators should not copy these 22 Qdrant-specific callbacks; instead,
/// define domain-specific callbacks only when they need Qdrant fields, or wait
/// for the shared observer shape from issue #36 when their events are generic.
///
/// [issue #36]: https://github.com/leynos/repovec-appliance/issues/36
pub trait QdrantQuadletObserver: Send + Sync {
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
    ///
    /// This callback remains available for implementations written before the
    /// general provisioning-dependency callback was added.
    fn missing_api_key_provisioning_dependency(
        &self,
        _directive: &str,
        _expected_dependency: &str,
    ) {
    }

    /// Records an incorrect API-key provisioning dependency.
    ///
    /// This callback remains available for implementations written before the
    /// general provisioning-dependency callback was added.
    fn incorrect_api_key_provisioning_dependency(
        &self,
        _directive: &str,
        _dependency: &str,
        _expected_dependency: &str,
    ) {
    }

    /// Records a missing required provisioning dependency.
    ///
    /// The default forwards the existing API-key dependency to its original
    /// callback, keeping implementations that only override that callback
    /// source-compatible. Other dependencies are ignored by default.
    fn missing_provisioning_dependency(&self, directive: &str, expected_dependency: &str) {
        if expected_dependency == QDRANT_API_KEY_SERVICE {
            self.missing_api_key_provisioning_dependency(directive, expected_dependency);
        }
    }

    /// Records an incorrect required provisioning dependency.
    fn incorrect_provisioning_dependency(
        &self,
        directive: &str,
        dependency: &str,
        expected_dependency: &str,
    ) {
        if expected_dependency == QDRANT_API_KEY_SERVICE {
            self.incorrect_api_key_provisioning_dependency(
                directive,
                dependency,
                expected_dependency,
            );
        }
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
