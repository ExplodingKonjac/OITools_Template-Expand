# Implementation Details

## Entry Points

| Crate | Entry | Mechanism |
|-------|-------|-----------|
| texpand-cli | `main()` in `main.rs` | `clap::Parser` to `expand()` |
| texpand-vscode | `main()` in `src/main.rs` | WASI process, env-var config to `expand()` to JSON stdout |
| VSCode TS | `extension.ts` | VSCode activation events to WASM lifecycle to result display |

## Request Trace (CLI)

```
cli/main.rs:main()
  → Cli::parse()                         # clap argument parsing
  → config::TexpandConfig::load()        # optional ~/.config/texpand.toml
  → FsResolver::new()                    # std::fs-backed resolver
  → expander::expand()                   # recursive BFS expansion
      → parser::parse_source()           # first parse
      → expand_recursive()              # DFS AST walk
          → classify_include()           # identify Local vs System includes
          → FileResolver::resolve()      # resolve #include path
          → FileResolver::read_content() # read included file
          → expand_recursive()           # recurse into dependency
      → CompressorState (optional)       # token-level compression
  → output (clipboard/file/stdout)
```

## Key Algorithms

### BFS Expansion with Preproc Context

In `expander.rs`, `expand_recursive()` walks the AST in DFS order. For each node:

1. **`preproc_include`**: resolve local includes via `FileResolver`, recurse, inline result. System includes preserved as-is.
2. **`#pragma once`**: stripped entirely.
3. **Compound conditionals** (`#ifdef`, `#ifndef`, `#if`): push `PreprocDirective` onto a context stack. The context tracks which conditional branch we are in.
4. **Dedup key**: `(file_path, PreprocContext)` — the same file can be expanded in different conditional contexts.

### Cycle Detection

Uses a `expanding: HashSet<PathBuf>` stack. If a file is encountered while already on the stack, the full cycle path is reported in the error. This is set-based, not graph-based — linear in expansion depth.

### Token-Level Compression

`compressor.rs` walks AST leaves only:
- Comments are skipped entirely (tree-sitter `"comment"` node kind)
- Space inserted between adjacent identifier characters (`[a-zA-Z0-9_]`)
- Before/after `#` preprocessor directives: force newline
- Compound preproc bodies (`#ifdef ... #endif`): insert newline before body
- `literal_suffix` nodes: no space inserted (preserves `123_km`)
- `#define` name field: trailing space forced (prevents `#define FOO"abc"`)

## Error Handling

- `anyhow::Result` throughout both CLI and WASM frontends
- `.with_context()` for descriptive error messages
- Cycle detection produces a human-readable path
- VSCode WASM wraps errors in JSON `{ success: false, error: "..." }` envelope

## Testing Strategy

- **Unit tests**: inline in each source file (`#[cfg(test)] mod tests`)
- **Integration tests**: `tests/` directory with `FixtureResolver` providing in-memory file content
- **Fixtures**: `fixtures/` directory with real C/C++ files for CLI end-to-end
- **Key test scenarios**:
  - Basic expansion / transitive dependencies / diamond deps
  - Circular dependency detection
  - Compression (comments, identifiers, preproc, defines, user-defined literals)
  - Conditional includes inside `#ifdef`/`#else`/`#endif` blocks
  - System include preservation
  - Edge cases: empty source, only comments, nested ifdefs

## Performance Considerations

- **AST caching**: Parsed trees cached in `HashMap<PathBuf, Tree>` to avoid re-parsing
- **Capacity hints**: Output strings pre-allocated with `String::with_capacity`
- **Release profile**: LTO + strip + panic=abort minimizes binary size
- **No unnecessary allocation**: `CompressorState` keeps a reusable buffer
