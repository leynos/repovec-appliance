//! A scripted loopback HTTP server that records the requests it receives.
//!
//! The listing adapters are ordinary blocking HTTP clients, so their protocol
//! contract is observed most directly by asking a real socket what they sent.
//! Each test scripts a sequence of responses; the server answers one request
//! per scripted entry, records each request, and answers anything beyond the
//! script with `500 Internal Server Error`. Answering rather than failing keeps
//! an unexpected extra request observable instead of hiding it behind a
//! transport error.
//!
//! The server listens on an ephemeral loopback port, so tests never contend for
//! a fixed port. Bind the origin into the adapter under test rather than into
//! production code: the production origin remains GitHub HTTPS.
//!
//! The module is split by concern: `expectation` holds the scripted responses
//! and recorded requests, `wire` reads and writes them on a connection, and
//! this file owns the listener and its worker thread. Both are private: what
//! they define is re-exported here, so callers see one module path.
//!
//! # Examples
//!
//! ```
//! use repovec_test_helpers::recording_http::{RecordingServer, ScriptedResponse};
//!
//! let server = RecordingServer::start(vec![ScriptedResponse::new(200).with_body("[]")])?;
//! let origin = server.origin();
//!
//! // A client bound to `origin` would now receive the scripted body.
//! assert!(origin.starts_with("http://127.0.0.1:"));
//! assert!(server.requests().is_empty());
//!
//! # Ok::<(), std::io::Error>(())
//! ```

mod expectation;
mod wire;

use std::{
    collections::VecDeque,
    io::{self, BufReader},
    net::{SocketAddr, TcpListener, TcpStream},
    sync::{
        Arc,
        Mutex,
        atomic::{AtomicBool, Ordering},
    },
    thread::{self, JoinHandle},
    time::Duration,
};

pub use expectation::{RecordedRequest, ScriptedResponse};
pub use wire::ORIGIN_TOKEN;
use wire::{read_request, write_response};

/// How long an idle connection is held open before the server gives up on it.
const CONNECTION_IDLE_TIMEOUT: Duration = Duration::from_secs(2);

/// How long the accept loop waits between checks of the shutdown flag.
const ACCEPT_POLL_INTERVAL: Duration = Duration::from_millis(2);

/// A loopback HTTP server that answers a script and records requests.
pub struct RecordingServer {
    /// Address the worker thread listens on.
    address: SocketAddr,
    /// State shared with the worker thread.
    shared: Arc<Shared>,
    /// Handle of the worker thread, taken when the server shuts down.
    worker: Option<JoinHandle<()>>,
}

impl RecordingServer {
    /// Starts a server that answers successive requests with `script`.
    ///
    /// # Errors
    ///
    /// Returns the I/O error when the loopback listener cannot be bound or the
    /// worker thread cannot be started.
    pub fn start(script: Vec<ScriptedResponse>) -> io::Result<Self> {
        let listener = TcpListener::bind("127.0.0.1:0")?;
        listener.set_nonblocking(true)?;
        let address = listener.local_addr()?;
        let shared = Arc::new(Shared {
            requests: Mutex::new(Vec::new()),
            is_stopped: AtomicBool::new(false),
        });
        let served = Arc::clone(&shared);
        let origin = format!("http://{address}");
        let worker = thread::spawn(move || run(&listener, &origin, script, &served));
        Ok(Self {
            address,
            shared,
            worker: Some(worker),
        })
    }

    /// Returns the server's origin, for example `http://127.0.0.1:34567`.
    #[must_use]
    pub fn origin(&self) -> String { format!("http://{}", self.address) }

    /// Returns every request recorded so far.
    ///
    /// A request is recorded before its response is written, so by the time a
    /// blocking client call returns the request it made is already visible.
    #[must_use]
    pub fn requests(&self) -> Vec<RecordedRequest> {
        self.shared
            .requests
            .lock()
            .map(|requests| requests.clone())
            .unwrap_or_default()
    }

    /// Returns the number of requests recorded so far.
    #[must_use]
    pub fn request_count(&self) -> usize {
        self.shared
            .requests
            .lock()
            .map(|requests| requests.len())
            .unwrap_or_default()
    }

    /// Stops the server and waits for its worker thread.
    ///
    /// Calling this more than once is harmless; the second call finds no thread
    /// to join.
    pub fn shutdown(&mut self) {
        self.shared.is_stopped.store(true, Ordering::SeqCst);
        if let Some(worker) = self.worker.take() {
            let _ignored = worker.join();
        }
    }
}

impl Drop for RecordingServer {
    fn drop(&mut self) { self.shutdown(); }
}

/// State shared between the server handle and its worker thread.
#[derive(Debug)]
struct Shared {
    /// Requests recorded so far.
    requests: Mutex<Vec<RecordedRequest>>,
    /// Whether the worker should stop.
    is_stopped: AtomicBool,
}

/// Accepts connections until the shutdown flag is set.
fn run(listener: &TcpListener, origin: &str, script: Vec<ScriptedResponse>, shared: &Shared) {
    let mut pending = VecDeque::from(script);
    while !shared.is_stopped.load(Ordering::SeqCst) {
        match listener.accept() {
            Ok((stream, _)) => serve(stream, origin, &mut pending, shared),
            Err(error) if error.kind() == io::ErrorKind::WouldBlock => {
                thread::sleep(ACCEPT_POLL_INTERVAL);
            }
            Err(_) => return,
        }
    }
}

/// Serves every request one connection sends, recording each one.
fn serve(
    stream: TcpStream,
    origin: &str,
    pending: &mut VecDeque<ScriptedResponse>,
    shared: &Shared,
) {
    if stream
        .set_read_timeout(Some(CONNECTION_IDLE_TIMEOUT))
        .is_err()
    {
        return;
    }
    let Ok(reader_stream) = stream.try_clone() else {
        return;
    };
    let mut reader = BufReader::new(reader_stream);
    let mut writer = stream;
    while let Some(request) = read_request(&mut reader) {
        if let Ok(mut recorded) = shared.requests.lock() {
            recorded.push(request);
        }
        let response = pending
            .pop_front()
            .unwrap_or_else(|| ScriptedResponse::new(500));
        if write_response(&mut writer, origin, &response).is_err() {
            return;
        }
    }
}
