# Technology Stack

## Language & Runtime

| Layer | Technology | Version | Rationale |
|-------|-----------|---------|-----------|
| Core library | Rust | edition 2024 | Performance, safety, WASM target support |
| CLI binary | Rust | same | No runtime overhead, easy distribution |
| VSCode extension | TypeScript + WASM | Node 22 | WASM-WASI integration, no native binary needed |

## Key Frameworks & Libraries

### texpand-core

| Library | Purpose | Why |
|---------|---------|-----|
| `tree-sitter` 0.24 | C/C++ parsing | Incremental, robust AST generation for real-world C/C++ code |
| `tree-sitter-cpp` 0.23 | C/C++ grammar | Supports preprocessor directives as AST nodes (essential for include analysis) |
| `serde` 1 | Serialization | Result JSON for VSCode frontend |

### texpand-cli

| Library | Purpose | Why |
|---------|---------|-----|
| `clap` 4 | CLI argument parsing | Derive-based, compile-time checks, subcommand support |
| `toml` 1 | Config file parsing | Standard format for `~/.config/texpand.toml` |
| `arboard` 3 | Clipboard access | Cross-platform clipboard (forks on Linux for persistence) |
| `nix` 0 | Fork syscall | Linux clipboard daemon forking on Linux |

### texpand-vscode

| Library | Purpose | Why |
|---------|---------|-----|
| `serde_json` 1 | JSON output | Structured result for TypeScript host |

(VSCode side uses `@vscode/wasm-wasi` for WASM process lifecycle and `esbuild` for bundling.)

## Storage

- **Config file**: `~/.config/texpand.toml` (TOML) — optional, CLI only
- **VSCode settings**: `settings.json` under `texpand.*` keys — extension side
- No database layer — the tool operates on ephemeral source files

## Infrastructure

- **CI**: GitHub Actions (push + PR triggers)
- **Release**: GitHub Releases with cross-platform CLI binaries + `.vsix`
- **WASI SDK**: wasi-sdk 33 for WASM compilation (wasm32-wasip1 target)
- **Cross-compilation**: gcc-aarch64-linux-gnu for ARM64 Linux builds
