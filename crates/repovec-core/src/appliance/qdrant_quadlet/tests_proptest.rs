//! Property-based tests covering the Qdrant Quadlet parser and validator
//! contract invariants that cannot be adequately exercised by example-based
//! tests alone.

use proptest::prelude::*;

use super::{QdrantQuadletError, parser::ParsedQuadlet, validate_qdrant_quadlet};

// --------------- Strategy helpers ---------------

/// Generates a syntactically valid image reference of the form
/// `registry/repo:tag` where the tag is not `latest`.
///
/// The regex excludes uppercase and special characters so that only
/// conventionally valid image name components are produced. The
/// `prop_filter` rejects `latest` as a tag because validator 1.2.1
/// requires an explicitly pinned tag.
///
/// Example: `docker.io/myrepo/myimage:1`
fn valid_image() -> impl Strategy<Value = String> {
    (r"[a-z]+\.[a-z]+", r"[a-z]+/[a-z0-9]+", "[a-z0-9]+")
        .prop_filter("tag must not be 'latest'", |(_, _, tag)| tag != "latest")
        .prop_map(|(registry, repo, tag)| format!("{registry}/{repo}:{tag}"))
}

/// Selects an IPv4 address that is not a loopback address
/// (`127.x.x.x`), covering public, private, and any-address forms.
///
/// The fixed sample set avoids generating loopback-segment addresses
/// (e.g., `127.0.0.1`) so the strategy always produces an address
/// that must be rejected by the loopback constraint.
///
/// Example: `192.168.1.1`
fn non_loopback_ip() -> impl Strategy<Value = String> {
    prop::sample::select(vec![
        "0.0.0.0".to_owned(),
        "192.168.1.1".to_owned(),
        "10.0.0.1".to_owned(),
        "172.16.0.1".to_owned(),
        "8.8.8.8".to_owned(),
        "203.0.113.1".to_owned(),
    ])
}

/// Generates a valid unprivileged TCP port number in the range
/// 1024–65534.
///
/// The lower bound excludes privileged ports (0–1023) which require
/// root; the upper bound is the maximum valid TCP port.
///
/// Example: `6333`
fn host_port() -> impl Strategy<Value = u16> { 1024_u16..65535 }

/// Generates an arbitrary lowercase alphabetic string representing a
/// candidate `AutoUpdate` policy value.
///
/// The regex is limited to `[a-z]+` so that the strategy produces only
/// simple, conventional-looking policy names without punctuation or
/// whitespace that would complicate Quadlet line parsing.
///
/// Example: `registry`
fn auto_update_policy() -> impl Strategy<Value = String> { "[a-z]+" }

/// Returns a complete, well-formed Quadlet file string that passes
/// `validate_qdrant_quadlet` without modification; used as the
/// baseline for injection-based tests.
///
/// The returned string is a verbatim copy of
/// `packaging/systemd/qdrant.container` and is the only Quadlet layout
/// accepted by the validator.
///
/// Example snippet:
/// ```text
/// [Container]
/// Image=docker.io/qdrant/qdrant:v1
/// AutoUpdate=registry
/// PublishPort=127.0.0.1:6333:6333
/// PublishPort=127.0.0.1:6334:6334
/// Volume=/var/lib/repovec/qdrant-storage:/qdrant/storage:Z
/// ```
fn valid_quadlet_base() -> String {
    String::from(
        "[Container]\n\
         Image=docker.io/qdrant/qdrant:v1\n\
         AutoUpdate=registry\n\
         PublishPort=127.0.0.1:6333:6333\n\
         PublishPort=127.0.0.1:6334:6334\n\
         Volume=/var/lib/repovec/qdrant-storage:/qdrant/storage:Z\n",
    )
}

proptest! {
    // ------ Parser invariance ------

    /// Verifies that arbitrary leading/trailing whitespace around the
    /// `Image` key and value does not cause validation to fail.
    #[test]
    fn whitespace_tolerance(
        key_pad in r"[ \t]{0,4}",
        val_pad in r"[ \t]{0,4}",
    ) {
        let contents = format!(
            "[Container]\n\
             {key_pad}Image{key_pad}={val_pad}docker.io/qdrant/qdrant:v1{val_pad}\n\
             AutoUpdate=registry\n\
             PublishPort=127.0.0.1:6333:6333\n\
             PublishPort=127.0.0.1:6334:6334\n\
             Volume=/var/lib/repovec/qdrant-storage:/qdrant/storage:Z\n"
        );
        validate_qdrant_quadlet(&contents)
            .expect("quadlet with whitespace around Image key/value should remain valid");
    }

    /// Verifies that injecting arbitrary comment lines into a valid
    /// Quadlet does not affect validation outcome.
    #[test]
    fn comment_injection_invariance(
        injections in prop::collection::vec(
            (r"# .*", 0_usize..10),
            0..=3,
        ),
    ) {
        let base = valid_quadlet_base();
        let mut lines: Vec<String> = base.lines().map(String::from).collect();
        for (comment, pos) in &injections {
            let idx = (*pos).min(lines.len());
            if comment.starts_with('#') {
                lines.insert(idx, comment.clone());
            } else {
                lines.insert(idx, String::from("# injected comment"));
            }
        }
        let contents = lines.join("\n") + "\n";
        validate_qdrant_quadlet(&contents)
            .expect("quadlet with injected comment lines should remain valid");
    }

    /// Verifies that inserting empty lines at any position in a valid
    /// Quadlet does not affect validation outcome.
    #[test]
    fn empty_line_invariance(
        pos in 0_usize..10,
        count in 0_usize..=4,
    ) {
        let base = valid_quadlet_base();
        let mut lines: Vec<String> = base.lines().map(String::from).collect();
        let idx = pos.min(lines.len());
        for _ in 0..count {
            lines.insert(idx, String::new());
        }
        let contents = lines.join("\n") + "\n";
        validate_qdrant_quadlet(&contents)
            .expect("quadlet with injected empty lines should remain valid");
    }

    /// Verifies that `ParsedQuadlet` preserves the declaration order of
    /// repeated keys under a section.
    #[test]
    fn key_accumulation_ordering(
        val1 in "[a-z]+",
        val2 in "[a-z]+",
        val3 in "[a-z]+",
    ) {
        prop_assume!(val1 != val2 && val2 != val3 && val1 != val3);
        let contents = format!(
            "[TestSection]\n\
             Key={val1}\n\
             Key={val2}\n\
             Key={val3}\n"
        );
        let parsed = ParsedQuadlet::parse(&contents)
            .expect("valid quadlet syntax should parse");
        let values = parsed.values("TestSection", "Key");
        prop_assert_eq!(values, &[val1.as_str(), val2.as_str(), val3.as_str()]);
    }

    /// Verifies that the parsed `Container/Image` value is identical
    /// regardless of whether the `[Container]` section appears before
    /// or after another section.
    #[test]
    fn section_ordering_invariance(
        extra_section in r"[A-Z][a-zA-Z]*",
        extra_key in r"[A-Z][a-zA-Z]*",
        extra_val in "[a-z]+",
    ) {
        prop_assume!(extra_section != "Container");
        let container_first = format!(
            "[Container]\n\
             Image=docker.io/qdrant/qdrant:v1\n\
             [{extra_section}]\n\
             {extra_key}={extra_val}\n"
        );
        let extra_first = format!(
            "[{extra_section}]\n\
             {extra_key}={extra_val}\n\
             [Container]\n\
             Image=docker.io/qdrant/qdrant:v1\n"
        );

        let parsed_cf = ParsedQuadlet::parse(&container_first)
            .expect("valid quadlet should parse");
        let parsed_ef = ParsedQuadlet::parse(&extra_first)
            .expect("valid quadlet should parse");

        prop_assert_eq!(
            parsed_cf.values("Container", "Image"),
            parsed_ef.values("Container", "Image")
        );
    }

    // ------ Duplicate entry rejection ------

    /// Verifies that any two `Image=` entries, whether their values are
    /// identical or distinct, cause `validate_qdrant_quadlet` to return
    /// `QdrantQuadletError::UnexpectedImage`.
    #[test]
    fn rejects_duplicate_images(
        image1 in valid_image(),
        image2 in valid_image(),
    ) {
        let contents = format!(
            "[Container]\n\
             Image={image1}\n\
             Image={image2}\n\
             AutoUpdate=registry\n\
             PublishPort=127.0.0.1:6333:6333\n\
             PublishPort=127.0.0.1:6334:6334\n\
             Volume=/var/lib/repovec/qdrant-storage:/qdrant/storage:Z\n"
        );
        let error = validate_qdrant_quadlet(&contents)
            .expect_err("duplicate Image= should be rejected");
        let expected = QdrantQuadletError::UnexpectedImage {
            image: format!("{image1},{image2}"),
        };
        prop_assert_eq!(error, expected);
    }

    /// Verifies that any two `AutoUpdate=` entries, whether their
    /// values are identical or distinct, cause `validate_qdrant_quadlet`
    /// to return `QdrantQuadletError::IncorrectAutoUpdate`.
    #[test]
    fn rejects_duplicate_auto_update(
        policy1 in auto_update_policy(),
        policy2 in auto_update_policy(),
    ) {
        let contents = format!(
            "[Container]\n\
             Image=docker.io/qdrant/qdrant:v1\n\
             AutoUpdate={policy1}\n\
             AutoUpdate={policy2}\n\
             PublishPort=127.0.0.1:6333:6333\n\
             PublishPort=127.0.0.1:6334:6334\n\
             Volume=/var/lib/repovec/qdrant-storage:/qdrant/storage:Z\n"
        );
        let error = validate_qdrant_quadlet(&contents)
            .expect_err("duplicate AutoUpdate= should be rejected");
        let expected = QdrantQuadletError::IncorrectAutoUpdate {
            auto_update: format!("{policy1},{policy2}"),
        };
        prop_assert_eq!(error, expected);
    }

    // ------ Port loopback constraint ------

    /// Verifies that a `PublishPort` binding for container port 6333
    /// to a non-loopback host IP is rejected with
    /// `QdrantQuadletError::PortNotBoundToLoopback`.
    #[test]
    fn port_6333_requires_loopback(
        host_ip in non_loopback_ip(),
        port in host_port(),
    ) {
        prop_assume!(host_ip != "127.0.0.1");
        let publish_port = format!("{host_ip}:{port}:6333");
        let contents = format!(
            "[Container]\n\
             Image=docker.io/qdrant/qdrant:v1\n\
             AutoUpdate=registry\n\
             PublishPort={publish_port}\n\
             PublishPort=127.0.0.1:6334:6334\n\
             Volume=/var/lib/repovec/qdrant-storage:/qdrant/storage:Z\n"
        );
        let error = validate_qdrant_quadlet(&contents)
            .expect_err("non-loopback port 6333 should be rejected");
        let expected = QdrantQuadletError::PortNotBoundToLoopback {
            port: 6333,
            publish_port,
        };
        prop_assert_eq!(error, expected);
    }

    /// Verifies that a `PublishPort` binding for container port 6334
    /// to a non-loopback host IP is rejected with
    /// `QdrantQuadletError::PortNotBoundToLoopback`.
    #[test]
    fn port_6334_requires_loopback(
        host_ip in non_loopback_ip(),
        port in host_port(),
    ) {
        prop_assume!(host_ip != "127.0.0.1");
        let publish_port = format!("{host_ip}:{port}:6334");
        let contents = format!(
            "[Container]\n\
             Image=docker.io/qdrant/qdrant:v1\n\
             AutoUpdate=registry\n\
             PublishPort=127.0.0.1:6333:6333\n\
             PublishPort={publish_port}\n\
             Volume=/var/lib/repovec/qdrant-storage:/qdrant/storage:Z\n"
        );
        let error = validate_qdrant_quadlet(&contents)
            .expect_err("non-loopback port 6334 should be rejected");
        let expected = QdrantQuadletError::PortNotBoundToLoopback {
            port: 6334,
            publish_port,
        };
        prop_assert_eq!(error, expected);
    }

    /// Verifies that a Quadlet containing one loopback and one
    /// non-loopback binding for the same container port (6333) is still
    /// rejected.
    #[test]
    fn loopback_and_nonloopback_coexistence_rejection(
        host_ip in non_loopback_ip(),
        port in host_port(),
    ) {
        prop_assume!(host_ip != "127.0.0.1");
        let bad_publish = format!("{host_ip}:{port}:6333");
        let contents = format!(
            "[Container]\n\
             Image=docker.io/qdrant/qdrant:v1\n\
             AutoUpdate=registry\n\
             PublishPort=127.0.0.1:6333:6333\n\
             PublishPort={bad_publish}\n\
             PublishPort=127.0.0.1:6334:6334\n\
             Volume=/var/lib/repovec/qdrant-storage:/qdrant/storage:Z\n"
        );
        let error = validate_qdrant_quadlet(&contents)
            .expect_err("coexisting non-loopback port 6333 should be rejected");
        let expected = QdrantQuadletError::PortNotBoundToLoopback {
            port: 6333,
            publish_port: bad_publish,
        };
        prop_assert_eq!(error, expected);
    }

    /// Verifies that a Quadlet containing one loopback and one
    /// non-loopback binding for the same container port (6334) is still
    /// rejected.
    #[test]
    fn loopback_and_nonloopback_coexistence_rejection_6334(
        host_ip in non_loopback_ip(),
        port in host_port(),
    ) {
        prop_assume!(host_ip != "127.0.0.1");
        let bad_publish = format!("{host_ip}:{port}:6334");
        let contents = format!(
            "[Container]\n\
             Image=docker.io/qdrant/qdrant:v1\n\
             AutoUpdate=registry\n\
             PublishPort=127.0.0.1:6333:6333\n\
             PublishPort=127.0.0.1:6334:6334\n\
             PublishPort={bad_publish}\n\
             Volume=/var/lib/repovec/qdrant-storage:/qdrant/storage:Z\n"
        );
        let error = validate_qdrant_quadlet(&contents)
            .expect_err("coexisting non-loopback port 6334 should be rejected");
        let expected = QdrantQuadletError::PortNotBoundToLoopback {
            port: 6334,
            publish_port: bad_publish,
        };
        prop_assert_eq!(error, expected);
    }
}
