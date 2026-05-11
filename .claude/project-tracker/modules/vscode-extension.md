# Module: texpand-vscode

**Path**: `texpand-vscode/`
**Type**: WASM binary + TypeScript extension
**Entry**: Rust `src/main.rs`, TypeScript `extension/src/extension.ts`

## Responsibility

VSCode extension that provides expand-in-place and expand-to-clipboard functionality for C/C++ files. Runs the core Rust engine as a WASI process — no native binary dependency.

## Rust WASM Layer (`src/main.rs`)

WASI process entry point. Reads configuration from environment variables:

| Env Var | Purpose |
|---------|---------|
| `TEXPAND_ENTRY_PATH` | Entry source file path |
| `TEXPAND_COMPRESS` | `"true"` to enable compression |
| `TEXPAND_INCLUDE_PATHS` | Comma-separated include search paths |

**WasiFsResolver**: Implements `FileResolver` using WASI filesystem access. Resolves includes relative to the includers directory, then against configured prefix paths.

**Output**: Writes JSON to stdout:
- Success: `{ "success": true, "data": "<expanded source>" }`
- Error: `{ "success": false, "error": "<error message>" }`

## TypeScript Extension (`extension/src/`)

### Files
| File | Purpose |
|------|---------|
| `extension.ts` | VSCode activation, command registration, result handling |
| `wasm.ts` | WASI process lifecycle management |

### Activation
Triggers on `onLanguage:c` and `onLanguage:cpp` events.

### Commands
| Command | Behavior |
|---------|----------|
| `texpand.expandDefault` | Uses `texpand.outputMode` setting |
| `texpand.expandAndCopy` | Forces clipboard output |
| `texpand.expandToNewFile` | Creates `*.expanded.cpp` file |

### WASM Lifecycle (`wasm.ts`)
1. Resolves WASM binary path relative to extension
2. Creates `WasiProcess` with mapped workspace directories
3. Sets environment variables from extension settings
4. Reads stdout until process exits
5. Parses JSON result and handles errors

### Settings (contributes to `package.json`)
| Key | Type | Default |
|-----|------|---------|
| `texpand.includePaths` | `string[]` | `["./"]` |
| `texpand.defaultCompression` | `boolean` | `false` |
| `texpand.outputMode` | `clipboard` or `newFile` | `"clipboard"` |
| `texpand.saveBeforeExpansion` | `boolean` | `true` |

## Build Pipeline

```
Cargo.toml
  ↓ cargo build --target wasm32-wasip1 --release
WASM binary (target/wasm32-wasip1/release/)
  ↓ wasm-opt
Optimized WASM (pkg/texpand-vscode.wasm)
  ↓ esbuild bundling
dist/extension.js
  ↓ vsce package
texpand-vscode-*.vsix
```

## Dependencies

### Rust
| Crate | Usage |
|-------|-------|
| `texpand-core` | Core expansion |
| `serde` + `serde_json` | JSON output |
| `anyhow` | Error handling |

### TypeScript
| Package | Usage |
|---------|-------|
| `@vscode/wasm-wasi` | WASI process host |
| `@vscode/vsce` | VSIX packaging |
| `esbuild` | TypeScript bundling |
