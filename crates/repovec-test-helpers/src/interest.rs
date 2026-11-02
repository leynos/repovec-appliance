//! Reliability for `tracing`'s global interest cache during scoped captures.
//!
//! A `tracing` callsite caches its interest process-wide and recomputes it
//! against the dispatchers that are alive. When exactly one dispatcher is
//! alive, `tracing-core` short-circuits that recomputation and asks the
//! *calling thread* for its dispatcher instead. A thread that never installed
//! one reports "no subscriber", whose callsite interest is `never`, so the
//! callsite is cached as `never` for every thread and its events are dropped
//! everywhere: including inside a concurrent capture whose own subscriber
//! would accept them.
//!
//! Test suites hit this because tests run concurrently in one process and only
//! some of them install a subscriber. [`anchor_interest_cache`] keeps one
//! always-interested dispatcher alive for the whole process, so a recomputation
//! always runs against the real dispatcher list rather than against a
//! thread-local default that the calling thread does not have.

use std::sync::OnceLock;

use tracing::{
    Dispatch, Event, Metadata, Subscriber,
    span::{Attributes, Id, Record},
};

/// Keeps `tracing`'s interest cache trustworthy for the rest of the process.
///
/// Call this before installing a scoped subscriber whose events must not be
/// dropped while other threads first use the same callsites. The anchor is
/// never made current on any thread, so it never formats an event; it only
/// takes part in the interest computation that decides whether a callsite is
/// enabled at all.
///
/// # Examples
///
/// ```
/// # use repovec_test_helpers::anchor_interest_cache;
/// anchor_interest_cache();
/// # let subscriber = tracing::subscriber::NoSubscriber::default();
/// tracing::subscriber::with_default(subscriber, || {});
/// ```
pub fn anchor_interest_cache() {
    static ANCHOR: OnceLock<Dispatch> = OnceLock::new();
    let _anchor = ANCHOR.get_or_init(|| Dispatch::new(AlwaysInterested));
}

/// A dispatcher that is interested in everything and records nothing.
///
/// It exists only to keep [`anchor_interest_cache`]'s dispatcher in the live
/// set; nothing is ever dispatched to it because it is never made current.
#[derive(Debug)]
struct AlwaysInterested;

impl Subscriber for AlwaysInterested {
    fn enabled(&self, _metadata: &Metadata<'_>) -> bool { true }

    fn new_span(&self, _span: &Attributes<'_>) -> Id { Id::from_u64(1) }

    fn record(&self, _span: &Id, _values: &Record<'_>) {}

    fn record_follows_from(&self, _span: &Id, _follows: &Id) {}

    fn event(&self, _event: &Event<'_>) {}

    fn enter(&self, _span: &Id) {}

    fn exit(&self, _span: &Id) {}
}

#[cfg(test)]
mod tests {
    use std::{sync::mpsc, thread};

    use super::anchor_interest_cache;
    use crate::{capture_logs, ensure_log_line_contains};

    /// The text of the probe event. It appears only in this module, so no
    /// other test's activity can satisfy the assertion below.
    const PROBE_MESSAGE: &str = "log capture interest probe";

    /// A callsite that another, subscriber-less thread first uses must still
    /// be captured here.
    ///
    /// Without [`anchor_interest_cache`] the first use comes from a thread
    /// with no dispatcher, so `tracing` caches the callsite as `never` and the
    /// event emitted below is dropped even though this thread's subscriber
    /// accepts it. That is the intermittent loss the anchored dispatcher
    /// prevents, and it is why the worker has to register the callsite while
    /// the capture is already running.
    #[test]
    fn callsite_first_used_by_a_subscriberless_thread_is_still_captured() {
        let (register_tx, register_rx) = mpsc::channel::<()>();
        let (done_tx, done_rx) = mpsc::channel::<()>();

        let ((), logs) = capture_logs(move || {
            let worker = thread::spawn(move || {
                register_rx.recv().expect("the capture should signal the worker to register");
                emit_probe();
                done_tx.send(()).expect("the worker should report completion");
            });
            register_tx.send(()).expect("the worker should be waiting to register");
            done_rx.recv().expect("the worker should finish registering the callsite");
            emit_probe();
            worker.join().expect("the worker should join");
        })
        .expect("capturing logs should succeed");

        ensure_log_line_contains(
            &logs,
            "DEBUG",
            PROBE_MESSAGE,
            "an event at a callsite first used by a subscriber-less thread should be captured",
        )
        .expect("the anchored interest cache should keep the probe event");
    }

    /// Emit the probe event from whichever thread calls this.
    fn emit_probe() {
        tracing::debug!("{}", PROBE_MESSAGE);
    }
}
