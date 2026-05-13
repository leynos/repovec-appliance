//! Process entry point for the repovec control-plane daemon.

use repovec_core::appliance::systemd_units::validate_checked_in_systemd_units;

fn main() {
    tracing_subscriber::fmt::init();
    validate_systemd_unit_contract();

    let _arguments = std::env::args_os();
}

fn validate_systemd_unit_contract() {
    if let Err(error) = validate_checked_in_systemd_units() {
        tracing::error!(error = %error, "systemd unit contract violation — aborting startup");
        std::process::exit(1);
    }

    tracing::debug!("systemd unit contract validated");
}
