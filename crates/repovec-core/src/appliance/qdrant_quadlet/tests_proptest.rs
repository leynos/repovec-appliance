//! Property-based tests covering the Qdrant Quadlet parser and validator
//! contract invariants that cannot be adequately exercised by example-based
//! tests alone.

use proptest::prelude::*;

use super::{QdrantQuadletError, parser::ParsedQuadlet, validate_qdrant_quadlet};

// --------------- Strategy helpers ---------------

fn valid_image() -> impl Strategy<Value = String> {
    (r"[a-z]+\.[a-z]+", r"[a-z]+/[a-z0-9]+", "[a-z0-9]+")
        .prop_filter("tag must not be 'latest'", |(_, _, tag)| tag != "latest")
        .prop_map(|(registry, repo, tag)| format!("{registry}/{repo}:{tag}"))
}

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

fn host_port() -> impl Strategy<Value = u16> { 1024_u16..65535 }

fn auto_update_policy() -> impl Strategy<Value = String> { "[a-z]+" }

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

    #[test]
    fn comment_injection_invariance(
        comments in prop::collection::vec(r"# .*", 0..=3),
    ) {
        let base = valid_quadlet_base();
        let mut lines: Vec<&str> = base.lines().collect();
        for comment in &comments {
            if comment.starts_with('#') {
                lines.push(comment.as_str());
            } else {
                lines.push("# injected comment");
            }
        }
        let contents = lines.join("\n") + "\n";
        validate_qdrant_quadlet(&contents)
            .expect("quadlet with trailing comment lines should remain valid");
    }

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

    #[test]
    fn rejects_duplicate_images(
        image1 in valid_image(),
        image2 in valid_image(),
    ) {
        prop_assume!(image1 != image2);
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

    #[test]
    fn rejects_duplicate_auto_update(
        policy1 in auto_update_policy(),
        policy2 in auto_update_policy(),
    ) {
        prop_assume!(policy1 != policy2);
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
}
