//! Shared proptest strategies for systemd unit parser and validator tests.

use proptest::prelude::*;

/// Returns the canonical checked-in `repovec.target` contents used as a
/// property-test baseline.
pub(super) fn valid_target_base() -> String {
    include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/../../packaging/systemd/repovec.target"))
        .to_owned()
}

/// Returns the canonical checked-in `repovecd.service` contents used as a
/// property-test baseline.
pub(super) fn valid_repovecd_base() -> String {
    include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/../../packaging/systemd/repovecd.service"))
        .to_owned()
}

/// Returns the canonical checked-in `repovec-mcpd.service` contents used as a
/// property-test baseline.
pub(super) fn valid_mcpd_base() -> String {
    include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../packaging/systemd/repovec-mcpd.service"
    ))
    .to_owned()
}

/// Returns valid `repovec.target` contents with `[Install]` before `[Unit]`.
pub(super) fn valid_target_install_first() -> String {
    concat!(
        "[Install]\n",
        "WantedBy=multi-user.target\n",
        "\n",
        "[Unit]\n",
        "Description=repovec appliance service group\n",
        "Wants=qdrant.service repovecd.service ",
        "repovec-mcpd.service cloudflared.service\n",
    )
    .to_owned()
}

/// Returns valid `repovecd.service` contents with `[Service]` before `[Unit]`.
pub(super) fn valid_repovecd_service_first() -> String {
    concat!(
        "[Service]\n",
        "Type=simple\n",
        "User=repovec\n",
        "Group=repovec\n",
        "WorkingDirectory=/var/lib/repovec\n",
        "Environment=HOME=/var/lib/repovec\n",
        "ExecStart=/usr/bin/repovecd\n",
        "Restart=on-failure\n",
        "\n",
        "[Unit]\n",
        "Description=repovec control-plane daemon\n",
        "Requires=qdrant.service\n",
        "After=qdrant.service\n",
    )
    .to_owned()
}

/// Generates valid insertion indices for edits against `base`, including the
/// tail position.
pub(super) fn insertion_position(base: &str) -> impl Strategy<Value = usize> {
    let line_count = base.lines().count();
    0..=line_count
}

/// Generates optional horizontal whitespace around parsed keys and values.
pub(super) fn whitespace() -> impl Strategy<Value = String> { r"[ \t]{0,4}" }

/// Generates a systemd comment line beginning with `#` or `;`.
pub(super) fn comment_line() -> impl Strategy<Value = String> {
    (prop::sample::select(vec!['#', ';']), r"[^\n]{0,40}")
        .prop_map(|(prefix, text)| format!("{prefix}{text}"))
}

/// Selects one of the rejected Quadlet source unit names.
pub(super) fn quadlet_source_name() -> impl Strategy<Value = String> {
    prop::sample::select(vec!["qdrant.container".to_owned(), "qdrant.container.service".to_owned()])
}

/// Selects valid systemd dependency names used by the repovec unit contract.
pub(super) fn valid_dependency() -> impl Strategy<Value = String> {
    prop::sample::select(vec![
        "qdrant.service".to_owned(),
        "repovecd.service".to_owned(),
        "repovec-mcpd.service".to_owned(),
        "cloudflared.service".to_owned(),
        "multi-user.target".to_owned(),
    ])
}
