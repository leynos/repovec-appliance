//! Appliance platform binding checks for the Qdrant Quadlet contract.
//!
//! Qdrant defines the container-facing requirements. This adapter validates how
//! the repovec appliance binds those requirements to the host: loopback port
//! publishing, persistent storage location, `SELinux` relabelling, and Podman
//! auto-update policy.

use super::{
    CONTAINER_SECTION, QdrantQuadletError, REQUIRED_GRPC_PORT, REQUIRED_REST_PORT,
    REQUIRED_STORAGE_TARGET, parser::ParsedQuadlet,
};

/// The host path where the appliance stores Qdrant's persistent data.
pub(super) const REQUIRED_STORAGE_SOURCE: &str = "/var/lib/repovec/qdrant-storage";
/// The REST API host binding for the appliance-managed Qdrant container.
///
/// Format: `IP:host_port:container_port`.
const REQUIRED_REST_BINDING: &str = "127.0.0.1:6333:6333";
/// The gRPC API host binding for the appliance-managed Qdrant container.
///
/// Format: `IP:host_port:container_port`.
const REQUIRED_GRPC_BINDING: &str = "127.0.0.1:6334:6334";
/// The Podman auto-update policy required for the appliance-managed service.
pub(super) const REQUIRED_AUTO_UPDATE_POLICY: &str = "registry";
/// The `SELinux` relabel option required for the host storage mount.
pub(super) const REQUIRED_SELINUX_OPTION: &str = "Z";

/// Validate appliance-owned platform bindings for the Qdrant container.
pub(super) fn validate_platform_bindings(parsed: &ParsedQuadlet) -> Result<(), QdrantQuadletError> {
    validate_required_port(
        parsed,
        REQUIRED_REST_PORT,
        REQUIRED_REST_BINDING,
        QdrantQuadletError::MissingRestPort,
    )?;
    validate_required_port(
        parsed,
        REQUIRED_GRPC_PORT,
        REQUIRED_GRPC_BINDING,
        QdrantQuadletError::MissingGrpcPort,
    )?;
    validate_storage_mount(parsed)?;
    validate_auto_update(parsed)
}

fn validate_required_port(
    parsed: &ParsedQuadlet,
    container_port: u16,
    required_binding: &str,
    missing_error: QdrantQuadletError,
) -> Result<(), QdrantQuadletError> {
    let publish_ports = parsed.values(CONTAINER_SECTION, "PublishPort");

    let matching_publish_ports = publish_ports
        .iter()
        .map(String::as_str)
        .filter(|port| published_container_port(port) == Some(container_port))
        .collect::<Vec<_>>();

    if matching_publish_ports.is_empty() {
        return Err(missing_error);
    }

    if let Some(publish_port) =
        matching_publish_ports.iter().find(|port| **port != required_binding)
    {
        return Err(QdrantQuadletError::PortNotBoundToLoopback {
            port: container_port,
            publish_port: (*publish_port).to_owned(),
        });
    }

    Ok(())
}

fn published_container_port(publish_port: &str) -> Option<u16> {
    let parts: Vec<_> = publish_port.split(':').collect();
    if parts.len() != 3 {
        return None;
    }

    parts.get(2)?.parse::<u16>().ok()
}

fn validate_storage_mount(parsed: &ParsedQuadlet) -> Result<(), QdrantQuadletError> {
    let volumes = parsed.values(CONTAINER_SECTION, "Volume");
    let Some((volume, parts)) = volumes.iter().find_map(|volume| storage_mount_candidate(volume))
    else {
        return Err(QdrantQuadletError::MissingStorageMount);
    };

    let Some(source) = parts.first().copied() else {
        return Err(QdrantQuadletError::MissingStorageMount);
    };
    if source != REQUIRED_STORAGE_SOURCE {
        return Err(QdrantQuadletError::IncorrectStorageSource { source: source.to_owned() });
    }

    let Some(target) = parts.get(1).copied() else {
        return Err(QdrantQuadletError::MissingStorageMount);
    };
    if target != REQUIRED_STORAGE_TARGET {
        return Err(QdrantQuadletError::IncorrectStorageTarget { target: target.to_owned() });
    }

    if !parts.get(2..).is_some_and(has_required_selinux_relabel_option) {
        return Err(QdrantQuadletError::MissingSelinuxRelabel { volume: volume.to_owned() });
    }

    Ok(())
}

fn has_required_selinux_relabel_option(options: &[&str]) -> bool {
    options
        .iter()
        .flat_map(|group| group.split(','))
        .map(str::trim)
        .any(|option| option.eq_ignore_ascii_case(REQUIRED_SELINUX_OPTION))
}

fn storage_mount_candidate(volume: &str) -> Option<(&str, Vec<&str>)> {
    let parts = volume.split(':').collect::<Vec<_>>();
    if parts.len() < 2 {
        return None;
    }

    let has_required_source =
        parts.first().is_some_and(|source| *source == REQUIRED_STORAGE_SOURCE);
    let has_required_target = parts.get(1).is_some_and(|target| *target == REQUIRED_STORAGE_TARGET);

    (has_required_source || has_required_target).then_some((volume, parts))
}

fn validate_auto_update(parsed: &ParsedQuadlet) -> Result<(), QdrantQuadletError> {
    let auto_updates = parsed.values(CONTAINER_SECTION, "AutoUpdate");
    let auto_update = match auto_updates {
        [] => return Err(QdrantQuadletError::MissingAutoUpdate),
        [auto_update] => auto_update,
        duplicate_auto_updates => {
            return Err(QdrantQuadletError::IncorrectAutoUpdate {
                auto_update: duplicate_auto_updates.join(","),
            });
        }
    };

    if auto_update != REQUIRED_AUTO_UPDATE_POLICY {
        return Err(QdrantQuadletError::IncorrectAutoUpdate { auto_update: auto_update.clone() });
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    //! Unit tests for appliance platform binding parsing helpers.

    use proptest::prelude::*;
    use rstest::rstest;

    use super::{
        REQUIRED_SELINUX_OPTION, REQUIRED_STORAGE_SOURCE, has_required_selinux_relabel_option,
        published_container_port, storage_mount_candidate,
    };

    #[rstest]
    #[case::rest_binding("127.0.0.1:6333:6333", Some(6333))]
    #[case::grpc_binding("127.0.0.1:6334:6334", Some(6334))]
    #[case::minimum_port("127.0.0.1:0:0", Some(0))]
    #[case::maximum_port("127.0.0.1:65535:65535", Some(65535))]
    #[case::too_few_fields("6333", None)]
    #[case::too_many_fields("127.0.0.1:6333:6333:tcp", None)]
    #[case::empty_container_port("127.0.0.1:6333:", None)]
    #[case::non_numeric_container_port("127.0.0.1:6333:http", None)]
    #[case::out_of_range_container_port("127.0.0.1:6333:65536", None)]
    fn published_container_port_accepts_only_three_field_numeric_mappings(
        #[case] publish_port: &str,
        #[case] expected: Option<u16>,
    ) {
        assert_eq!(published_container_port(publish_port), expected);
    }

    #[rstest]
    #[case::required_source("/var/lib/repovec/qdrant-storage:/other", true)]
    #[case::required_target("/other:/qdrant/storage", true)]
    #[case::required_source_and_target("/var/lib/repovec/qdrant-storage:/qdrant/storage:Z", true)]
    #[case::too_few_parts("/var/lib/repovec/qdrant-storage", false)]
    #[case::unrelated_mount("/tmp/other:/tmp/target:Z", false)]
    #[case::empty("", false)]
    fn storage_mount_candidate_matches_required_source_or_target(
        #[case] volume: &str,
        #[case] expected: bool,
    ) {
        assert_eq!(storage_mount_candidate(volume).is_some(), expected);
    }

    #[test]
    fn storage_mount_candidate_returns_original_volume_and_split_parts() {
        let volume = "/var/lib/repovec/qdrant-storage:/qdrant/storage:rw,Z";
        let (candidate, parts) =
            storage_mount_candidate(volume).expect("required storage mount should match");

        assert_eq!(candidate, volume);
        assert_eq!(parts, vec!["/var/lib/repovec/qdrant-storage", "/qdrant/storage", "rw,Z"]);
    }

    #[test]
    fn has_required_selinux_relabel_option_matches_split_trimmed_case_insensitive_tokens() {
        let cases: &[(&[&str], bool)] = &[
            (&[], false),
            (&["rw"], false),
            (&["Y"], false),
            (&["Z"], true),
            (&["z"], true),
            (&["rw", "Z"], true),
            (&["rw,Z"], true),
            (&["rw, z"], true),
            (&["rw, z,ro"], true),
        ];

        for (options, expected) in cases {
            assert_eq!(
                has_required_selinux_relabel_option(options),
                *expected,
                "options: {options:?}",
            );
        }
    }

    proptest! {
        #[test]
        fn published_container_port_returns_any_valid_u16_container_port(
            host in "[^:]*",
            host_port in "[^:]*",
            container_port in any::<u16>(),
        ) {
            let publish_port = format!("{host}:{host_port}:{container_port}");

            prop_assert_eq!(published_container_port(&publish_port), Some(container_port));
        }

        #[test]
        fn published_container_port_rejects_mappings_without_three_fields(
            fields in prop::collection::vec("[^:]*", 0..8)
                .prop_filter("field count must not be three", |fields| fields.len() != 3),
        ) {
            let publish_port = fields.join(":");

            prop_assert_eq!(published_container_port(&publish_port), None);
        }

        #[test]
        fn storage_mount_candidate_rejects_values_with_too_few_parts(
            fields in prop::collection::vec("[^:]*", 0..2),
        ) {
            let volume = fields.join(":");

            prop_assert!(storage_mount_candidate(&volume).is_none());
        }

        #[test]
        fn storage_mount_candidate_accepts_any_required_source_mount(
            target in "[^:]*",
            options in prop::collection::vec("[^:]*", 0..4),
        ) {
            let mut fields = vec![REQUIRED_STORAGE_SOURCE.to_owned(), target];
            fields.extend(options);
            let volume = fields.join(":");

            prop_assert!(storage_mount_candidate(&volume).is_some());
        }

        #[test]
        fn storage_mount_candidate_accepts_any_required_target_mount(
            source in "[^:]*",
            options in prop::collection::vec("[^:]*", 0..4),
        ) {
            let mut fields = vec![source, super::REQUIRED_STORAGE_TARGET.to_owned()];
            fields.extend(options);
            let volume = fields.join(":");

            prop_assert!(storage_mount_candidate(&volume).is_some());
        }

        #[test]
        fn storage_mount_candidate_rejects_unrelated_mounts(
            source in "[^:]*",
            target in "[^:]*",
            options in prop::collection::vec("[^:]*", 0..4),
        ) {
            prop_assume!(source != REQUIRED_STORAGE_SOURCE);
            prop_assume!(target != super::REQUIRED_STORAGE_TARGET);

            let mut fields = vec![source, target];
            fields.extend(options);
            let volume = fields.join(":");

            prop_assert!(storage_mount_candidate(&volume).is_none());
        }

        #[test]
        fn has_required_selinux_relabel_option_matches_case_and_whitespace_variants(
            before in "[ \\t]{0,8}",
            after in "[ \\t]{0,8}",
            uppercase in any::<bool>(),
            prefix in prop::collection::vec("[A-Ya-y0-9]{0,8}", 0..4),
            suffix in prop::collection::vec("[A-Ya-y0-9]{0,8}", 0..4),
        ) {
            let relabel_token = if uppercase {
                REQUIRED_SELINUX_OPTION.to_owned()
            } else {
                REQUIRED_SELINUX_OPTION.to_ascii_lowercase()
            };
            let relabel = format!("{before}{relabel_token}{after}");
            let mut tokens = prefix;
            tokens.push(relabel);
            tokens.extend(suffix);
            let group = tokens.join(",");

            prop_assert!(has_required_selinux_relabel_option(&[group.as_str()]));
        }

        #[test]
        fn has_required_selinux_relabel_option_rejects_options_without_relabel_token(
            options in prop::collection::vec("[A-Ya-y0-9, \\t]{0,16}", 0..8),
        ) {
            let option_refs = options.iter().map(String::as_str).collect::<Vec<_>>();

            prop_assert!(!has_required_selinux_relabel_option(&option_refs));
        }
    }
}
