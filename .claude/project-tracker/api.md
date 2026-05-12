# API Surface

## CLI (`texpand`)

### Command
```
texpand [OPTIONS] <INPUT>
    INPUT: Path to C/C++ source file (use "-" for stdin)
```

### Options

| Flag | Description |
|------|-------------|
| `-c, --compress` | Enable token-level compression (overrides config) |
| `--no-compress` | Disable compression (overrides config) |
| `-i, --include <PATH>` | Add include search path (repeatable, overrides config) |
| `-o, --output <FILE>` | Write to file instead of stdout |
| `-C, --clipboard` | Copy to clipboard (mutually exclusive with `-o`) |
| `--config <FILE>` | Config file path (default: `~/.config/texpand.toml`) |

### Exit Codes
- 0: Success
- Non-zero: Error (anyhow prints diagnostic to stderr)

## VSCode Extension

### Commands
| Command ID | Title | Behavior |
|-----------|-------|----------|
| `texpand.expandDefault` | Texpand: Expand Current File (Default) | Uses configured `outputMode` |
| `texpand.expandAndCopy` | Texpand: Expand and Copy | Forces clipboard output |
| `texpand.expandToNewFile` | Texpand: Expand to New File | Creates `.expanded.cpp` |

### Activation
Events: `onLanguage:c`, `onLanguage:cpp`

### WASM Process Protocol
The TypeScript extension:
1. Spawns a WASI process from the WASM binary
2. Sets env vars: `TEXPAND_ENTRY_PATH`, `TEXPAND_COMPRESS`, `TEXPAND_INCLUDE_PATHS`
3. Mounts workspace filesystem paths
4. Reads JSON from WASM stdout: `{ success: bool, data?: string, error?: string }`
5. Displays result (clipboard or new file)

## Core Library (`texpand-core`)

### Public API
```rust
// expander.rs
pub fn expand(
    entry_path: &Path,
    entry_source: &str,
    resolver: &dyn FileResolver,
    opts: &ExpandOptions,
) -> Result<String>

pub struct ExpandOptions {
    pub compress: bool,
}

pub trait FileResolver {
    fn resolve(&self, includer_path: &Path, include_path: &str) -> Result<PathBuf>;
    fn read_content(&self, resolved_path: &Path) -> Result<String>;
}

// Standalone compression
pub fn compress(tree: &Tree, source: &str) -> String;
pub fn compress_stripped(tree: &Tree, source: &str) -> String;
```

No REST API, no HTTP endpoints — this is a local-only tool.
