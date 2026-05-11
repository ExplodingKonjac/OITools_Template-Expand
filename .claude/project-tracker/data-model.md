# Data Model

## Core Types

### `FileResolver` trait (resolver.rs)
```
FileResolver
├── resolve(includer_path, include_path) to Result<PathBuf>
└── read_content(resolved_path) to Result<String>
```

The central I/O boundary. Two implementations:
- `FsResolver` (CLI): `std::fs::canonicalize` + `read_to_string`
- `WasiFsResolver` (VSCode): WASI filesystem paths

### Include types (parser.rs)
```
Include<'a>
├── Local(&'a str)    // #include "path"
└── System(&'a str)   // #include <path>
```

### Preproc directive stack (expander.rs)
```
PreprocDirective
├── If(Subject)
├── Ifdef(Subject)
├── Ifndef(Subject)
├── Elif(Subject)
├── Elifdef(Subject)
└── Else

PreprocContext(Vec<PreprocDirective>)
```

The `Subject` is a `Vec<String>` — the token sequence extracted from the conditionals argument for structural equivalence comparison.

### ExpandState (expander.rs)
```
ExpandState
├── completed: HashSet<(PathBuf, PreprocContext)>
│     Files + context pairs already expanded (dedup key)
├── expanding: HashSet<PathBuf>
│     Cycle detection stack
└── tree_cache: HashMap<PathBuf, Tree>
│     Parsed AST cache
```

### CompressorState (compressor.rs)
```
CompressorState
├── output: String              // Accumulated compressed output
├── prev_last: Option<char>     // Last emitted char (identifier spacing)
├── compound_depth: usize       // #ifdef / #if nesting depth
└── body_nl_counter: Option<usize>
      Sibling counter for body newline insertion
```

## Config Model

### CLI config (texpand-cli/config.rs)
```toml
# ~/.config/texpand.toml
include_paths = ["./templates", "~/algo/cpp_lib"]
default_compress = false
```

### VSCode settings
| Key | Type | Default |
|-----|------|---------|
| `texpand.includePaths` | `string[]` | `["./"]` |
| `texpand.defaultCompression` | `boolean` | `false` |
| `texpand.outputMode` | `"clipboard"` or `"newFile"` | `"clipboard"` |
| `texpand.saveBeforeExpansion` | `boolean` | `true` |

## VSCode Result Envelope
```
ExpandResult (JSON to stdout)
├── success: bool
├── data: Option<String>     // Expanded output
└── error: Option<String>    // Error message
```

## Key Invariants

- `texpand-core` must never call `std::fs` or `std::io` — all file data enters through `FileResolver`
- `PreprocContext` is purely structural — no actual macro evaluation; same token sequence = same context
- Dedup key `(PathBuf, PreprocContext)` means the same file CAN appear multiple times if included under different `#ifdef` branches
