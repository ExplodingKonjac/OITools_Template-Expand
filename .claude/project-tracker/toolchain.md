# Toolchain

## Build System

Cargo workspace with 3 members. Release profile: LTO + panic=abort + strip.

```bash
# Build all crates
cargo build --workspace

# Build specific crate
cargo build -p texpand-core
cargo build -p texpand-cli
cargo build -p texpand-vscode --target wasm32-wasip1 --release
```

## Linting & Formatting

- **Formatter**: `cargo fmt --all` (rustfmt, 100-char line width, 4-space indent)
- **Linter**: `cargo clippy --workspace -- -D warnings` (deny warnings as errors)

## Testing

- **Framework**: built-in `#[test]` with `#[cfg(test)]` modules
- **Unit tests**: inline in each source file under `mod tests`
- **Integration tests**: `texpand-core/tests/` — use `FixtureResolver` (in-memory file map)
- **Coverage target**: not explicitly configured (no coverage CI step)
- **Run**: `cargo test --workspace`

```
texpand-core/tests/
├── common.rs                     # Shared test utilities (FixtureResolver)
├── test_basic_expansion.rs
├── test_circular_dep.rs
├── test_compression.rs
├── test_conditional_includes.rs
├── test_edge_cases.rs
└── test_system_include.rs
```

## CI/CD Pipeline

### CI (`.github/workflows/ci.yml`)
Triggers on push to any branch and PRs:

| Job | Tool | Purpose |
|-----|------|---------|
| `check` | `cargo check` | Verify compilation |
| `fmt` | `cargo fmt -- --check` | Formatting compliance |
| `clippy` | `cargo clippy -- -D warnings` | Lint enforcement |
| `test` | `cargo test --workspace` | All tests |
| `typecheck` | `tsc --noEmit` | VSCode extension types |

### Release (`.github/workflows/release.yml`)
Triggers on `v*` tag push:

- **CLI binaries**: 5 targets (x86_64 Linux/ARM64 Linux/x86_64 macOS/ARM64 macOS/x86_64 Windows)
- **VSIX package**: WASM build + esbuild bundle + `vsce package`
- **GitHub Release**: auto-generated release notes, all artifacts attached

## Dev Environment Prerequisites

| Tool | Purpose |
|------|---------|
| Rust stable toolchain | Core compilation |
| wasm32-wasip1 target | VSCode WASM build |
| wasi-sdk 33 | WASM C/C++ linker |
| Node.js 22 | VSCode extension build |
| wasm-opt | WASM binary optimization |
| cargo-llvm-cov (optional) | Coverage reporting |

## Environment Variables

| Variable | Used By | Purpose |
|----------|---------|---------|
| `TEXPAND_ENTRY_PATH` | texpand-vscode WASM | Entry source file path |
| `TEXPAND_COMPRESS` | texpand-vscode WASM | Enable compression flag |
| `TEXPAND_INCLUDE_PATHS` | texpand-vscode WASM | Comma-separated include search paths |
| `XDG_CONFIG_HOME` | texpand-cli | Config file location override |
| `CC_wasm32_wasip1` | .cargo/config.toml | WASI SDK clang path |
| `CXX_wasm32_wasip1` | .cargo/config.toml | WASI SDK clang++ path |
