# Deployment

## Build Artifacts

### CLI Binary
- **Format**: Single platform-native executable
- **Targets**: x86_64 Linux, ARM64 Linux, x86_64 macOS, ARM64 macOS, x86_64 Windows
- **Build**: `cargo build -p texpand-cli --release --target <target>`
- **Size**: ~3-5 MB per binary (stripped, LTO)
- **Install**: `cargo install --path texpand-cli` or download from GitHub Releases

### VSCode Extension
- **Format**: `.vsix` package
- **Contents**: WASM binary (wasi target) + bundled TypeScript
- **Build**:
  1. `cargo build -p texpand-vscode --target wasm32-wasip1 --release`
  2. `wasm-opt` for optimization
  3. `esbuild` for TypeScript bundling
  4. `vsce package` for .vsix
- **Published**: GitHub Releases (not on Marketplace)

## Release Process

Trigger: push tag `v*` to GitHub.

```
Release workflow (release.yml):
├── build-cli (matrix: 5 targets)
│   ├── Install Rust + cross-compilers
│   ├── cargo build --release
│   ├── Rename to texpand-<target>
│   └── Upload artifact
├── build-vsix (ubuntu-latest)
│   ├── Install WASI SDK 33
│   ├── npm ci
│   ├── npm run vscode:prepublish  (WASM + esbuild)
│   ├── vsce package
│   └── Upload artifact
└── release (after both complete)
    ├── Download all artifacts
    └── Create GitHub Release with generated notes
```

## Environments

| Environment | Distribution | Update Mechanism |
|-------------|-------------|-----------------|
| Local dev | `cargo build` | Manual rebuild |
| End-user CLI | GitHub Releases / `cargo install` | Manual |
| VSCode | `.vsix` sideload | Manual install from VSIX |

## Health Checks & Monitoring

N/A — local CLI tool with no server component. The VSCode extension writes diagnostic messages to stderr (visible in output channel). No telemetry or crash reporting.

## Rollback

- CLI: reinstall previous version via `cargo install --version <old>` or download old release binary
- VSCode: Install from VSIX with previous version, or uninstall/reinstall
