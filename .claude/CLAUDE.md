# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Build & Test Commands

```bash
# Build entire workspace
cargo build --workspace

# Run all tests
cargo test --workspace

# Run a single test
cargo test test_name

# Run tests for a specific crate
cargo test -p texpand-core

# Run integration tests only
cargo test --test test_basic_expansion

# Format and lint
cargo fmt --all
cargo clippy --workspace -- -D warnings

# VSCode extension typecheck (from texpand-vscode/extension/)
npm run typecheck

# Build WASM binary for VSCode extension
cargo build -p texpand-vscode --target wasm32-wasip1 --release

# Full VSCode extension build (WASM + esbuild + package into .vsix)
npm run vscode:prepublish  # from texpand-vscode/extension/
npm run package            # create .vsix via vsce
```

## Project Architecture

Monorepo with 3 crates — a core Rust library with two frontends (CLI + VSCode extension via WASM).

### `texpand-core` (I/O-free core library)
The central processing engine. **Must never call `std::fs`/`std::io` directly** — all file reading goes through the `FileResolver` trait.

- **`resolver.rs`** — `FileResolver` trait: `resolve()` + `read_content()`. CLI implements via `std::fs`, VSCode via WASI filesystem.
- **`parser.rs`** — tree-sitter C/C++ parser wrapper. `parse_source()` → AST tree, `extract_all_includes()` → Local/System include classification.
- **`expander.rs`** — BFS-based recursive expansion. Tracks `PreprocContext` (conditional directive stack) for correct dedup inside `#ifdef`/`#if` branches. Walks the AST, processes `preproc_include`, `#pragma once`, compound conditionals.
- **`compressor.rs`** — Token-level compressor via AST leaf walk. Drops comments, inserts space between adjacent identifier chars, forces newlines around preproc directives. `CompressorState` is a reusable state machine.

### `texpand-cli` (CLI frontend)
- `clap`-based argument parsing.
- `FsResolver` implements `FileResolver` with `std::fs`.
- Config from `~/.config/texpand.toml` (`include_paths`, `default_compress`).
- Clipboard support via `arboard` (forks on Linux for persistence).

### `texpand-vscode` (VSCode extension frontend)
- **Rust WASM layer** (`src/main.rs`): WASI process entry point. Reads env vars (`TEXPAND_ENTRY_PATH`, `TEXPAND_COMPRESS`, `TEXPAND_INCLUDE_PATHS`), calls `texpand-core` expand, prints JSON result to stdout.
- **TypeScript extension** (`extension/`): Activates on C/C++ files. 3 commands: expandDefault, expandAndCopy, expandToNewFile. Loads WASM via `@vscode/wasm-wasi`, mounts workspace files, reads result.

### Tests
- **Unit tests**: `#[cfg(test)] mod tests` inside each source file in `texpand-core`.
- **Integration tests**: `texpand-core/tests/` — use `FixtureResolver` (in-memory file map implementing `FileResolver`).
- **Fixtures**: `fixtures/` directory with real C/C++ files for CLI end-to-end testing.

## Key Constraints

- `texpand-core` must stay I/O-free — no `std::fs`, no `std::io`. All data comes through `FileResolver`.
- Clippy must pass with `-D warnings`.
- Rust edition 2024; let-chains style preferred.
- The `graph.rs` module described in ARCHITECTURE.md was inlined into `expander.rs` — the dependency graph and cycle detection live there now.

## Localization

VSCode extension uses `@vscode/l10n`. Localized strings in `l10n/bundle.l10n.{locale}.json`. Package manifest strings in `package.nls.{locale}.json`. To export strings for translation: `npm run l10n:export` from `texpand-vscode/extension/`.
