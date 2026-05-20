//! Shared proptest strategies for Qdrant Quadlet parser and validator tests.

use proptest::prelude::*;

/// Generates a syntactically valid image reference of the form
/// `registry/repo:tag` where the tag is not `latest`.
///
/// The regex excludes uppercase and special characters so that only
/// conventionally valid image name components are produced. The
/// `prop_filter` rejects `latest` as a tag because validator 1.2.1
/// requires an explicitly pinned tag.
///
/// Example: `abc.def/repo/name123:tag456`
pub(super) fn valid_image() -> impl Strategy<Value = String> {
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
pub(super) fn non_loopback_ip() -> impl Strategy<Value = String> {
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
/// 1024-65535.
///
/// The lower bound excludes privileged ports (0-1023) which require
/// root; the upper bound is the maximum valid TCP port.
///
/// Example: `6333`
pub(super) fn host_port() -> impl Strategy<Value = u16> { 1024_u16..=65535 }

/// Generates an arbitrary lowercase alphabetic string representing a
/// candidate `AutoUpdate` policy value.
///
/// The regex is limited to `[a-z]+` so that the strategy produces only
/// simple, conventional-looking policy names without punctuation or
/// whitespace that would complicate Quadlet line parsing.
///
/// Example: `registry`
pub(super) fn auto_update_policy() -> impl Strategy<Value = String> { "[a-z]+" }

/// Returns a complete, well-formed Quadlet file string that passes
/// `validate_qdrant_quadlet` without modification; used as the
/// baseline for injection-based tests.
///
/// The returned string is a verbatim copy of
/// `packaging/systemd/qdrant.container` and represents one Quadlet
/// layout accepted by the validator.
///
/// Example snippet:
/// ```text
/// [Unit]
/// Requires=repovec-qdrant-api-key.service
/// After=repovec-qdrant-api-key.service
///
/// [Container]
/// Image=docker.io/qdrant/qdrant:v1
/// AutoUpdate=registry
/// Secret=repovec-qdrant-api-key,type=env,target=QDRANT__SERVICE__API_KEY
/// PublishPort=127.0.0.1:6333:6333
/// PublishPort=127.0.0.1:6334:6334
/// Volume=/var/lib/repovec/qdrant-storage:/qdrant/storage:Z
/// ```
pub(super) fn valid_quadlet_base() -> String {
    String::from(
        "[Unit]\n\
         Requires=repovec-qdrant-api-key.service\n\
         After=repovec-qdrant-api-key.service\n\
         \n\
         [Container]\n\
         Image=docker.io/qdrant/qdrant:v1\n\
         AutoUpdate=registry\n\
         Secret=repovec-qdrant-api-key,type=env,target=QDRANT__SERVICE__API_KEY\n\
         PublishPort=127.0.0.1:6333:6333\n\
         PublishPort=127.0.0.1:6334:6334\n\
         Volume=/var/lib/repovec/qdrant-storage:/qdrant/storage:Z\n",
    )
}

/// Generates valid insertion indices for generated Quadlet edits.
///
/// The range is based on `valid_quadlet_base()` and includes the tail
/// position. If it has 3 lines, this yields indices `0..=3`.
///
/// Use this to pick where to insert generated Quadlet edits.
pub(super) fn insertion_position() -> impl Strategy<Value = usize> {
    let line_count = valid_quadlet_base().lines().count();
    0..=line_count
}
