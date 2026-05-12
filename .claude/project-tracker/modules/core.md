# Module: texpand-core

**Path**: `texpand-core/`
**Type**: Library crate (I/O-free core)
**Entry**: `src/lib.rs` — re-exports 4 public modules

## Responsibility

Central processing engine for C/C++ include expansion. Must never call `std::fs` or `std::io` directly — all file I/O goes through the `FileResolver` trait.

## Modules

| Module | File | Lines | Key Items |
|--------|------|-------|-----------|
| `resolver` | `resolver.rs` | 26 | `FileResolver` trait |
| `parser` | `parser.rs` | 131 | `parse_source()`, `Include`, `extract_all_includes()` |
| `expander` | `expander.rs` | 525 | `expand()`, `ExpandOptions`, `PreprocContext`, `ExpandState` |
| `compressor` | `compressor.rs` | 552 | `CompressorState`, `compress()`, `compress_stripped()` |

## Dependencies

```
tree-sitter 0.24
tree-sitter-cpp 0.23  (C/C++ grammar for preproc-aware AST)
serde 1 + derive       (for ExpandResult serialization)
anyhow 1               (error propagation)
```

## Unique Patterns

- **PreprocContext dedup**: The `completed` set is `HashSet<(PathBuf, PreprocContext)>` — files are NOT deduplicated globally, only within the same preprocessor conditional context. This is the core correctness guarantee.
- **Trait-based I/O isolation**: `FileResolver` is the only way data enters the core. The trait has exactly two methods: `resolve()` (path resolution) and `read_content()` (file content).
- **Reusable CompressorState**: The state machine tracks `prev_last` char for identifier spacing, `compound_depth` for preproc nesting, and `body_nl_counter` for body newline insertion.

## Tests

- Unit tests inline in each module under `#[cfg(test)]`
- Integration tests in `tests/` using `FixtureResolver` (in-memory file map)
- Test data in `fixtures/` (real C/C++ files for CLI e2e)
