//! Unit tests for appliance platform binding parsing helpers.

use std::sync::atomic::{AtomicUsize, Ordering};

use proptest::prelude::*;
use rstest::rstest;

use super::{
    REQUIRED_SELINUX_OPTION, REQUIRED_STORAGE_SOURCE, has_required_selinux_relabel_option,
    published_container_port, storage_mount_candidate, validate_storage_mount,
};
use crate::appliance::qdrant_quadlet::{observer::QdrantQuadletObserver, parser::ParsedQuadlet};

#[derive(Default)]
struct StorageMountObserver {
    missing_mounts: AtomicUsize,
    missing_volumes: AtomicUsize,
}

impl QdrantQuadletObserver for StorageMountObserver {
    fn missing_storage_mount(&self, _expected_source: &str, _expected_target: &str) {
        self.missing_mounts.fetch_add(1, Ordering::Relaxed);
    }

    fn missing_storage_mount_volume(
        &self,
        _volume: &str,
        _expected_source: &str,
        _expected_target: &str,
    ) {
        self.missing_volumes.fetch_add(1, Ordering::Relaxed);
    }
}

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
#[case::required_source_without_target("/var/lib/repovec/qdrant-storage", true)]
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
fn validate_storage_mount_accepts_later_valid_candidate() {
    let parsed = ParsedQuadlet::parse(
        "\
[Container]
Volume=/var/lib/repovec/qdrant-storage:/wrong-target:Z
Volume=/other-source:/qdrant/storage:Z
Volume=/var/lib/repovec/qdrant-storage:/qdrant/storage:rw,Z
",
        &(),
    )
    .expect("quadlet fixture should parse");

    assert_eq!(validate_storage_mount(&parsed, &()), Ok(()));
}

#[test]
fn validate_storage_mount_reports_one_terminal_missing_volume_callback() {
    let observer = StorageMountObserver::default();
    let parsed = ParsedQuadlet::parse(
        "\
[Container]
Volume=/var/lib/repovec/qdrant-storage
",
        &(),
    )
    .expect("quadlet fixture should parse");

    assert!(validate_storage_mount(&parsed, &observer).is_err());
    assert_eq!(observer.missing_mounts.load(Ordering::Relaxed), 0);
    assert_eq!(observer.missing_volumes.load(Ordering::Relaxed), 1);
}

#[test]
fn has_required_selinux_relabel_option_matches_split_trimmed_case_sensitive_tokens() {
    let cases: &[(&[&str], bool)] = &[
        (&[], false),
        (&["rw"], false),
        (&["Y"], false),
        (&["Z"], true),
        (&["z"], false),
        (&["rw", "Z"], true),
        (&["rw,Z"], true),
        (&["rw, z"], false),
        (&["rw, z,ro"], false),
    ];

    for (options, expected) in cases {
        assert_eq!(has_required_selinux_relabel_option(options), *expected, "options: {options:?}");
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
        prop_assume!(volume != REQUIRED_STORAGE_SOURCE);

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
        let mut fields = vec![source, super::super::REQUIRED_STORAGE_TARGET.to_owned()];
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
        prop_assume!(target != super::super::REQUIRED_STORAGE_TARGET);

        let mut fields = vec![source, target];
        fields.extend(options);
        let volume = fields.join(":");

        prop_assert!(storage_mount_candidate(&volume).is_none());
    }

    #[test]
    fn has_required_selinux_relabel_option_matches_exact_case_and_whitespace_variants(
        before in "[ \\t]{0,8}",
        after in "[ \\t]{0,8}",
        prefix in prop::collection::vec("[A-Ya-y0-9]{0,8}", 0..4),
        suffix in prop::collection::vec("[A-Ya-y0-9]{0,8}", 0..4),
    ) {
        let relabel = format!("{before}{REQUIRED_SELINUX_OPTION}{after}");
        let mut tokens = prefix;
        tokens.push(relabel);
        tokens.extend(suffix);
        let group = tokens.join(",");

        prop_assert!(has_required_selinux_relabel_option(&[group.as_str()]));
    }

    #[test]
    fn has_required_selinux_relabel_option_rejects_lowercase_relabel_token(
        before in "[ \\t]{0,8}",
        after in "[ \\t]{0,8}",
        prefix in prop::collection::vec("[A-Ya-y0-9]{0,8}", 0..4),
        suffix in prop::collection::vec("[A-Ya-y0-9]{0,8}", 0..4),
    ) {
        let relabel = format!("{before}{}{after}", REQUIRED_SELINUX_OPTION.to_ascii_lowercase());
        let mut tokens = prefix;
        tokens.push(relabel);
        tokens.extend(suffix);
        let group = tokens.join(",");

        prop_assert!(!has_required_selinux_relabel_option(&[group.as_str()]));
    }

    #[test]
    fn has_required_selinux_relabel_option_rejects_options_without_relabel_token(
        options in prop::collection::vec("[A-Ya-y0-9, \\t]{0,16}", 0..8),
    ) {
        let option_refs = options.iter().map(String::as_str).collect::<Vec<_>>();

        prop_assert!(!has_required_selinux_relabel_option(&option_refs));
    }
}
