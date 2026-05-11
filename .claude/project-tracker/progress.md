# Progress

## Current Phase

v0.2.0 — Stable release with dual CLI + VSCode support.

## Completed Features

- [x] Tree-sitter based C/C++ parsing with preprocessor support
- [x] Local `#include "..."` expansion via BFS with cycle detection
- [x] System `#include <...>` preservation in output
- [x] `#pragma once` stripping during expansion
- [x] Context-sensitive dedup: same file in different `#ifdef` branches re-expanded correctly
- [x] Token-level semantic-safe compression (comment removal, identifier spacing, preproc newlines)
- [x] User-defined literal preservation in compressor (`123_km` stays intact)
- [x] `#define` space preservation (macro name to replacement separation)
- [x] CLI frontend with clap (stdin, file output, clipboard, config file)
- [x] Linux clipboard fork daemon for paste persistence
- [x] VSCode extension via WASM-WASI (3 commands, virtual filesystem, l10n)
- [x] Cross-platform CI (5 targets) + VSCode VSIX packaging
- [x] Config file (`~/.config/texpand.toml`) with include paths
- [x] l10n support with Chinese locale

## Known Issues & Technical Debt

- No `-isystem` vs `-I` distinction in include resolution
- No diagnostic output mode (e.g., `--verbose` showing which files were expanded)
- Compression edge: some C++ constructs may need manual review (e.g., string literal concatenation)
- No benchmark suite for expansion performance
- Test fixtures are hand-written — could benefit from fuzzing

## Roadmap

### Near-term (v0.3.0)
- [ ] Support for `-isystem` include paths (system vs local include resolution)
- [ ] Verbose/diagnostic output mode
- [ ] Recursive directory scanning for include paths

### Medium-term
- [ ] Support for CMake `compile_commands.json` include path extraction
- [ ] `#pragma once` vs include guard heuristic detection
- [ ] WASM size optimization

### Future
- [ ] Language server protocol (LSP) integration
- [ ] Online Judge auto-submit feature
- [ ] Emacs integration
