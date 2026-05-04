<div align="center">

# texpand-vscode

**English** | [简体中文](https://github.com/ExplodingKonjac/OITools_Template-Expand/blob/main/texpand-vscode/extension/README.zh-CN.md)

![icon](./texpand-vscode/extension/assets/icon.png)

</div>

`texpand-vscode` is a VSCode extension that expands all local `#include` dependencies in C/C++ source files into a single, self-contained file. It optionally performs token-level code compression while preserving semantics.

The extension runs the core Rust logic via WebAssembly (WASI) — no local CLI tool or Rust toolchain required.

## Features

- **One-click expansion**: Expand all local `#include` headers directly in the editor — via the editor title button, context menu, or command palette.
- **Safe code compression**: Optionally strip comments and unnecessary whitespace with semantic-aware token merging that won't break your code.
- **Zero external dependency**: The WASM engine is bundled with the extension. No binary tools or Rust environment needed.
- **Remote development ready**: Works seamlessly with Remote-SSH, Dev Containers, and VSCode for Web via the virtual file system.
- **Flexible output**: Copy expanded code to clipboard, or write to a `.expanded.cpp` file alongside the original.

## Usage

After installing, open any C/C++ file and:

1. **Editor title bar** — click the file icon button in the top-right corner to expand and copy to clipboard.
2. **Context menu** — right-click the editor, choose an action from the **Texpand** submenu.
3. **Command palette** — press `Ctrl+Shift+P` / `Cmd+Shift+P` and type `Texpand`.
4. **Status bar** — click the **Texpand** button at the bottom-right to toggle compression or output mode.

## Extension Settings

Search for `texpand` in VSCode settings:

| Setting | Type | Default | Description |
| :--- | :--- | :--- | :--- |
| `texpand.includePaths` | `string[]` | `["./"]` | Header search paths. Supports workspace-relative or absolute paths. |
| `texpand.defaultCompression` | `boolean` | `false` | Whether to enable code compression by default. |
| `texpand.outputMode` | `"clipboard"` / `"newFile"` | `"clipboard"` | Output destination: clipboard, or a new `.expanded.cpp` file. |

## Commands

| Command | Description |
| :--- | :--- |
| **Texpand: Expand Current File (Default)** | Expand using the configured output mode. |
| **Texpand: Expand and Copy to Clipboard** | Expand and copy the result to clipboard. |
| **Texpand: Expand to New File** | Expand and write the result to a new file. |

## Known Issues

None yet.

## Release Notes

### 0.1.0

Initial release. Basic expansion, compression, clipboard and file output.
