# Template-Expand

C/C++ `#include` template expansion tool for Competitive Programming. Expands local headers into a single file, with optional semantic-safe token-level compression. Ships as both a CLI tool (`texpand`) and a VSCode extension.

## Table of Contents

- [Index](INDEX.md) — this file
- [Stack](stack.md) — technology choices & rationale
- [Toolchain](toolchain.md) — build, lint, test, CI/CD
- [Architecture](architecture.md) — module layout & data flow
- [Progress](progress.md) — current status & roadmap
- [Implementation](implementation.md) — entry points & key logic
- [Data Model](data-model.md) — core types & state
- [API](api.md) — CLI surface & extension commands
- [Deployment](deployment.md) — building & packaging
- [Modules](modules/) — per-crate deep dives
  - [Core](modules/core.md) — `texpand-core` library
  - [CLI](modules/cli.md) — `texpand-cli` frontend
  - [VSCode Extension](modules/vscode-extension.md) — `texpand-vscode` frontend

## Tech Stack Summary

- **Language**: Rust 2024 edition (stable toolchain)
- **Parser**: tree-sitter C/C++ grammar for AST-based include analysis
- **CLI**: clap argument parsing, `~/.config/texpand.toml` config
- **VSCode**: WASM-WASI process mode via `@vscode/wasm-wasi`, TypeScript extension host
- **Serialization**: serde/serde_json for structured output

## Quick-Reference Commands

```bash
# Build
cargo build --workspace

# Test
cargo test --workspace

# Lint
cargo clippy --workspace -- -D warnings

# Format
cargo fmt --all

# Run CLI
cargo run -p texpand-cli -- main.cpp -c

# Build VSCode WASM
cargo build -p texpand-vscode --target wasm32-wasip1 --release
```
