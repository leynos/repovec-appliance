//! Compile-time tests for the public Qdrant Quadlet observer contract.

#[test]
fn qdrant_quadlet_observer_requires_send_sync() {
    let test_cases = trybuild::TestCases::new();
    test_cases.compile_fail("tests/ui/qdrant_quadlet_observer_requires_send_sync.rs");
}
