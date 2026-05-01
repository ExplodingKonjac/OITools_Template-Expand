//! texpand-vscode: VSCode extension WASM frontend (Process mode).
//!
//! Launched by @vscode/wasm-wasi as a WASI process. Reads input JSON from
//! stdin, accesses workspace files via WASI filesystem (`std::fs`), and
//! writes result JSON to stdout.

use std::io::Read;

use anyhow::Result;
use serde::{Deserialize, Serialize};
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

#[derive(Deserialize)]
struct ExpandInput {
    entry_path: String,
    include_paths: Vec<String>,
    compress: bool,
}

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
    let mut input = String::new();
    if std::io::stdin().read_to_string(&mut input).is_err() {
        println!("{}", error_json("failed to read stdin".into()));
        return;
    }

    let input: ExpandInput = match serde_json::from_str(&input) {
        Ok(i) => i,
        Err(e) => {
            println!("{}", error_json(format!("invalid input JSON: {e}")));
            return;
        }
    };

    let entry_source = match std::fs::read_to_string(&input.entry_path) {
        Ok(s) => s,
        Err(e) => {
            println!("{}", error_json(format!("failed to read entry file: {e}")));
            return;
        }
    };

    let resolver = WasiFsResolver {
        include_paths: input.include_paths,
    };
    let opts = ExpandOptions {
        compress: input.compress,
    };

    let output = match expand(&input.entry_path, &entry_source, &resolver, &opts) {
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
