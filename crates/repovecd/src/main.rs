//! Process entry point for the repovec control-plane daemon.
//!
//! This binary initializes the process-wide `tracing` subscriber and delegates
//! startup contract validation to `repovec-core`.

fn main() {
    tracing_subscriber::fmt::init();
    if let Err(error) = repovec_core::appliance::daemon_startup::run_daemon_startup() {
        std::process::exit(error);
    }
}
