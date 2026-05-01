//! texpand-vscode: VSCode extension WASM frontend (Process mode).
//!
//! Launched by @vscode/wasm-wasi as a WASI process. Parameters passed via
//! environment variables. Accesses workspace files via WASI filesystem
//! (`std::fs`). Writes result JSON to stdout.

use anyhow::Result;
use serde::Serialize;
use texpand_core::expander::{ExpandOptions, expand};
use texpand_core::resolver::FileResolver;

// ── File resolver (uses std::fs via WASI) ─────────────────────────────────────

struct WasiFsResolver {
    include_paths: Vec<String>,
}

impl FileResolver for WasiFsResolver {
    fn resolve_and_read(
        &self,
        includer_path: &str,
        include_path: &str,
    ) -> Result<(String, String)> {
        let path = std::path::Path::new(include_path);

        if path.is_absolute() {
            let content = std::fs::read_to_string(path)?;
            return Ok((path.to_string_lossy().to_string(), content));
        }

        if let Some(parent) = std::path::Path::new(includer_path).parent() {
            let candidate = parent.join(path);
            if candidate.exists() {
                let content = std::fs::read_to_string(&candidate)?;
                return Ok((candidate.to_string_lossy().to_string(), content));
            }
        }

        for prefix in &self.include_paths {
            let candidate = std::path::Path::new(prefix).join(path);
            if candidate.exists() {
                let content = std::fs::read_to_string(&candidate)?;
                return Ok((candidate.to_string_lossy().to_string(), content));
            }
        }

        anyhow::bail!("texpand: file not found in workspace: {}", include_path)
    }
}

// ── Input / output ───────────────────────────────────────────────────────────

#[derive(Serialize)]
struct ExpandResult {
    success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    data: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<String>,
}

// ── Entry point ──────────────────────────────────────────────────────────────

pub fn main() {
    let entry_path = match std::env::var("TEXPAND_ENTRY_PATH") {
        Ok(p) => p,
        Err(e) => {
            println!("{}", error_json(format!("TEXPAND_ENTRY_PATH not set: {e}")));
            return;
        }
    };

    let compress = std::env::var("TEXPAND_COMPRESS")
        .map(|v| v == "true")
        .unwrap_or(false);

    let include_paths: Vec<String> = std::env::var("TEXPAND_INCLUDE_PATHS")
        .unwrap_or_default()
        .split(',')
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
        .collect();

    eprintln!("[texpand] entry_path={}", entry_path);
    eprintln!("[texpand] compress={}", compress);
    eprintln!("[texpand] include_paths={:?}", include_paths);

    let entry_source = match std::fs::read_to_string(&entry_path) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("[texpand] failed to read entry file: {e}");
            println!("{}", error_json(format!("failed to read entry file: {e}")));
            return;
        }
    };

    let resolver = WasiFsResolver { include_paths };
    let opts = ExpandOptions { compress };

    let output = match expand(&entry_path, &entry_source, &resolver, &opts) {
        Ok(data) => ExpandResult {
            success: true,
            data: Some(data),
            error: None,
        },
        Err(e) => ExpandResult {
            success: false,
            data: None,
            error: Some(e.to_string()),
        },
    };

    println!(
        "{}",
        serde_json::to_string(&output).unwrap_or_else(|e| format!(
            r#"{{"success":false,"error":"serialization error: {e}"}}"#
        ))
    );
}

fn error_json(msg: String) -> String {
    serde_json::to_string(&ExpandResult {
        success: false,
        data: None,
        error: Some(msg),
    })
    .unwrap_or_else(|e| format!(r#"{{"success":false,"error":"{e}"}}"#))
}
