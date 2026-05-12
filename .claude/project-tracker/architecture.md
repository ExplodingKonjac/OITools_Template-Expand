# Architecture

## Overview

Monorepo with a shared core library and two frontends. The core is I/O-free — all file reading goes through the `FileResolver` trait, allowing each frontend to provide its own storage backend.

```
┌──────────────────────────────────────────────────────┐
│                   texpand-core                       │
│  ┌──────────┐  ┌──────────┐  ┌──────────────────┐   │
│  │ parser   │  │ expander │  │ compressor       │   │
│  │ (tree-   │  │ (BFS     │  │ (token-level AST │   │
│  │  sitter) │  │  expand) │  │  leaf walk)      │   │
│  └────┬─────┘  └────┬─────┘  └────────┬─────────┘   │
│       │              │                  │             │
│  ┌────┴──────────────┴──────────────────┴─────────┐  │
│  │ resolver.rs (FileResolver trait)                │  │
│  │ resolve() + read_content() — no std::fs calls   │  │
│  └────────────────────┬────────────────────────────┘  │
└───────────────────────┼──────────────────────────────┘
                        │
          ┌─────────────┼─────────────┐
          │             │             │
┌─────────┴──────┐ ┌───┴────────┐ ┌──┴────────────┐
│  texpand-cli   │ │ texpand-   │ │ texpand-      │
│  FsResolver    │ │ vscode     │ │ vscode        │
│  (std::fs)     │ │ (WASI fs)  │ │ (TypeScript)  │
│                │ │            │ │               │
│  clap CLI      │ │ env-var    │ │ VSCode        │
│  config/toml   │ │ config     │ │ commands      │
│  arboard clip  │ │ JSON stdout│ │ WASM launcher │
└────────────────┘ └────────────┘ └───────────────┘
```

## Module Breakdown (texpand-core)

| Module | File | Responsibility |
|--------|------|---------------|
| `resolver` | `resolver.rs` | `FileResolver` trait — abstract file I/O |
| `parser` | `parser.rs` | tree-sitter C/C++ parse + include extraction |
| `expander` | `expander.rs` | BFS recursive expansion, cycle detection, preproc context tracking |
| `compressor` | `compressor.rs` | Token-level AST leaf walk compression |

## Key Data Flow

```
Source file ──→ parser.rs ──→ AST tree
                  │
                  ▼
          extract_includes()
                  │
                  ▼
          expander.rs ──→ FileResolver.resolve()
                  │            │
                  ▼            ▼
          Recursive expand ──→ read_content()
                  │
                  ▼
          Compressor (optional) ──→ Final string output
```

## Security Boundaries

- **I/O boundary**: `FileResolver` trait is the sole I/O interface — no `std::fs` or `std::io` in `texpand-core`
- **WASM sandbox**: VSCode extension runs in WASI process with virtual filesystem — no access to host filesystem beyond workspace
- **Fork isolation**: Linux clipboard uses `fork()` to persist clipboard content — child process exits independently

## Design Patterns

- **Trait-based abstraction**: `FileResolver` enables two completely different I/O backends (std::fs, WASI) from the same core
- **BFS with context-sensitive dedup**: `PreprocContext` stack tracks conditional directive state so `#include` inside `#ifdef`/`#else` branches is correctly handled
- **Reusable state machine**: `CompressorState` can be reused across multiple tree walks, tracking compound directive nesting and body newline insertion
