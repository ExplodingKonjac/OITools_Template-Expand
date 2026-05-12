# Module: texpand-cli

**Path**: `texpand-cli/`
**Type**: Binary crate (CLI frontend)
**Entry**: `src/main.rs` — `fn main()`

## Responsibility

Provides the command-line interface for texpand. Reads files from the local filesystem via `FsResolver`, handles config from `~/.config/texpand.toml`, and manages output (stdout, file, clipboard).

## Key Components

### `FsResolver` (main.rs:54-89)
Implementation of `FileResolver` using `std::fs`:
- Absolute paths: resolved directly via `canonicalize()`
- Relative paths: first checked relative to includer directory, then against configured `include_paths`
- `read_content()`: delegates to `std::fs::read_to_string()`

### CLI Parser (main.rs:15-48)
Clap derive-based parser with flags:
- `-c`/`--compress` and `--no-compress` — overrides config
- `-i`/`--include` — search paths (repeatable, overrides config)
- `-o`/`--output` — file output
- `-C`/`--clipboard` — clipboard output (mutually exclusive with `-o`)
- `--config` — custom config path
- Stdin support via `-` as input path

### Clipboard (main.rs:91-118)
- **Linux**: forks a child process that holds the clipboard data alive (arboard clipboard vanishes when process exits)
- **Other platforms**: direct `set_text()` call

### Config (config.rs)
`TexpandConfig` deserialized from TOML:
- `include_paths: Vec<String>`
- `default_compress: bool`
- Located at `~/.config/texpand.toml` (respects `XDG_CONFIG_HOME`)

## Dependencies

| Crate | Usage |
|-------|-------|
| `texpand-core` | Core expansion library |
| `clap 4` | CLI argument parsing |
| `serde 1` | Config deserialization |
| `toml 1` | Config file parsing |
| `arboard 3` | Clipboard access |
| `nix 0` | Linux fork syscall |
