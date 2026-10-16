# repovec nursery crates

This nested Cargo workspace contains candidate abstractions that require real
production evidence before publication. All packages set `publish = false` and
may make breaking changes without a compatibility period.

The production workspace excludes this directory. Run its gates explicitly:

```console
cargo fmt --manifest-path crates/nursery/Cargo.toml --all -- --check
cargo check --manifest-path crates/nursery/Cargo.toml --workspace --all-targets
cargo clippy --manifest-path crates/nursery/Cargo.toml --workspace \
  --all-targets -- -D warnings
cargo test --manifest-path crates/nursery/Cargo.toml --workspace --all-targets
cargo doc --manifest-path crates/nursery/Cargo.toml --workspace --no-deps
```

The dedicated workflow runs these gates on the repository toolchain and checks
the workspace separately on the declared Rust 1.85 minimum.

See `docs/nursery-crates.md` and ADR 004 for interface ownership, adoption
constraints, and graduation criteria.
