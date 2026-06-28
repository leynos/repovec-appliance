//! Test helpers for capturing `tracing` output.

use std::{
    io::{self, Write},
    string::FromUtf8Error,
    sync::{Arc, Mutex},
};

use thiserror::Error;
use tracing::Level;

pub(crate) fn capture_info_logs<F, R>(action: F) -> Result<(R, String), CaptureLogsError>
where
    F: FnOnce() -> R,
{
    let captured = Arc::new(Mutex::new(Vec::new()));
    let writer = CapturedWriter(Arc::clone(&captured));
    let subscriber = tracing_subscriber::fmt()
        .with_max_level(Level::INFO)
        .with_ansi(false)
        .without_time()
        .with_writer(move || writer.clone())
        .finish();

    let result = tracing::subscriber::with_default(subscriber, action);
    let bytes = captured.lock().map_err(|_| CaptureLogsError::Poisoned)?.clone();
    let logs = String::from_utf8(bytes)?;
    Ok((result, logs))
}

#[derive(Debug, Error)]
pub(crate) enum CaptureLogsError {
    #[error("captured tracing output mutex was poisoned")]
    Poisoned,
    #[error("captured tracing output was not UTF-8")]
    NonUtf8(#[from] FromUtf8Error),
}

#[derive(Clone, Debug)]
struct CapturedWriter(Arc<Mutex<Vec<u8>>>);

impl Write for CapturedWriter {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        let mut captured = self
            .0
            .lock()
            .map_err(|_| io::Error::other("captured tracing output mutex was poisoned"))?;
        captured.extend_from_slice(buf);
        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> { Ok(()) }
}
