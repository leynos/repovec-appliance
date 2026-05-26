//! Process entry point for the repovec MCP bridge daemon.

fn main() {
    tracing_subscriber::fmt::init();
    if let Err(error) =
        repovec_core::appliance::systemd_units::validate_and_trace_checked_in_units()
    {
        tracing::error!(
            unit = %error.unit(),
            error = %error,
            "systemd unit contract violation — aborting startup",
        );
        std::process::exit(1);
    }

    let _arguments = std::env::args_os();
}
