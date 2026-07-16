//! Compile-fail fixture proving `QdrantApiKey` secret material stays private.

use repovec_core::appliance::qdrant_liveness::QdrantApiKey;

fn main() {
    let key = QdrantApiKey::parse("qdrant-api-key").expect("test key should parse");
    let _secret = key.as_secret();
}
