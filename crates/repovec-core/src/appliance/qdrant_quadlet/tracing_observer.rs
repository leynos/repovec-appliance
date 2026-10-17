//! Tracing adapter for Qdrant Quadlet validation events.
//!
//! This module keeps production telemetry formatting separate from the
//! observer port, allowing consumers to implement the port without pulling
//! tracing-specific behaviour into their adapters.

use tracing::{error, info, warn};

use super::{LOG_TARGET, QDRANT_API_KEY_SERVICE, QdrantQuadletObserver};

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

    fn missing_provisioning_dependency(&self, directive: &str, expected_dependency: &str) {
        if expected_dependency == QDRANT_API_KEY_SERVICE {
            self.missing_api_key_provisioning_dependency(directive, expected_dependency);
            return;
        }
        warn!(
            target: LOG_TARGET,
            directive,
            expected_dependency,
            "qdrant quadlet validation failed: missing provisioning dependency"
        );
    }

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
            return;
        }
        warn!(
            target: LOG_TARGET,
            directive,
            dependency,
            expected_dependency,
            "qdrant quadlet validation failed: incorrect provisioning dependency"
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
