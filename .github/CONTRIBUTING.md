# Contributing

Build with Rust 1.95 or newer. Keep contributions small and show the behavior they change.

```sh
cargo build --locked -p pdf-editor-offline-cli -p pdf-editor-offline-native
cargo fmt --all -- --check
cargo test --workspace --locked
cargo test -p pdf-worker --features test-helper --locked
cargo clippy --workspace --all-targets --locked -- -D warnings
```

Run the affected crate while iterating. Rendering and document transformations also need independent output checks; see [reference setup](../docs/RUST_BENCHMARKS.md). A unit test alone does not establish PDF fidelity.

- Use synthetic public fixtures. Never attach a private document.
- Preserve the original and verify exported copies with another reader.
- State unsupported structures, losses, and prerequisites explicitly.
- Review actual desktop interaction and keyboard access for UI changes.
- Update [native status](../docs/RUST_STATUS.md), limitations, and migration evidence with the implementation.

See [architecture](../docs/ARCHITECTURE.md), [fixture policy](../docs/FIXTURE_POLICY.md), and [security reporting](SECURITY.md). Contributions are licensed under MIT.
