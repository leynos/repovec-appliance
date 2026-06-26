//! Compile-fail fixture proving `QdrantQuadletObserver` requires `Send + Sync`.

use std::rc::Rc;

use repovec_core::appliance::qdrant_quadlet::QdrantQuadletObserver;

struct NotThreadSafeObserver {
    _state: Rc<()>,
}

impl QdrantQuadletObserver for NotThreadSafeObserver {}

fn main() {}
