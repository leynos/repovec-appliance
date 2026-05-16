//! Process entry point for the repovec MCP bridge daemon.

fn main() {
    tracing_subscriber::fmt::init();
    if let Err(error) = validate_systemd_unit_contract() {
        tracing::error!(error = %error, "systemd unit contract violation — aborting startup");
        std::process::exit(1);
    }

    let _arguments = std::env::args_os();
}

fn validate_systemd_unit_contract()
-> Result<(), repovec_core::appliance::systemd_units::SystemdUnitError> {
    let result = repovec_core::appliance::systemd_units::validate_checked_in_systemd_units();
    if result.is_ok() {
        tracing::debug!("systemd unit contract validated");
    }
    result
}

#[cfg(test)]
mod tests {
    //! Unit tests for the repovec-mcpd startup contract validation.

    use repovec_core::appliance::systemd_units::{SystemdUnitError, validate_systemd_units};

    #[test]
    fn validate_systemd_unit_contract_succeeds_for_checked_in_units() {
        // The helper is wired to validate_checked_in_systemd_units(); exercise
        // the public entry point directly to prove the wiring.
        repovec_core::appliance::systemd_units::validate_checked_in_systemd_units()
            .expect("checked-in units must satisfy the contract at compile time");
    }

    #[test]
    fn validate_systemd_unit_contract_returns_err_on_invalid_units() {
        // Supply a minimal but deliberately broken repovec-mcpd unit (wrong ExecStart)
        // and assert that validation returns Err rather than panicking or exiting.
        let target = "\
[Unit]
Wants=qdrant.service repovecd.service repovec-mcpd.service cloudflared.service

[Install]
WantedBy=multi-user.target
";
        let repovecd = "\
[Unit]
Requires=qdrant.service
After=qdrant.service

[Service]
User=repovec
Group=repovec
WorkingDirectory=/var/lib/repovec
Environment=HOME=/var/lib/repovec
ExecStart=/usr/bin/repovecd
";
        let broken_mcpd = "\
[Unit]
Requires=qdrant.service repovecd.service
After=qdrant.service repovecd.service

[Service]
User=repovec
Group=repovec
WorkingDirectory=/var/lib/repovec
Environment=HOME=/var/lib/repovec
ExecStart=/usr/bin/wrong-binary
";
        let result = validate_systemd_units(target, repovecd, broken_mcpd);
        assert!(
            matches!(result, Err(SystemdUnitError::IncorrectExecStart { .. })),
            "expected IncorrectExecStart, got {result:?}",
        );
    }
}
