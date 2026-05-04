//! Shared utilities for integration tests.
//!
//! Cargo compiles every `.rs` in `tests/` as a test binary — `common.rs` has
//! no `#[test]` functions so it produces an empty runner, which is harmless.

#![allow(dead_code)]

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use anyhow::Result;
use texpand_core::expander::{ExpandOptions, expand};
use texpand_core::resolver::FileResolver;

/// Mock resolver backed by an in-memory file map.
pub struct FixtureResolver {
    files: HashMap<String, String>,
}

impl FixtureResolver {
    pub fn new(entries: impl IntoIterator<Item = (&'static str, &'static str)>) -> Self {
        let files = entries
            .into_iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect();
        Self { files }
    }

    pub fn empty() -> Self {
        Self {
            files: HashMap::new(),
        }
    }
}

impl FileResolver for FixtureResolver {
    fn resolve(&self, _includer_path: &Path, path: &str) -> Result<PathBuf> {
        self.files
            .contains_key(path)
            .then(|| path.into())
            .ok_or_else(|| anyhow::anyhow!("file not found: {path}"))
    }
    fn read_content(&self, path: &Path) -> Result<String> {
        self.files
            .get(path.to_string_lossy().as_ref())
            .cloned()
            .ok_or_else(|| anyhow::anyhow!("file not found: {}", path.display()))
    }
}

pub fn expand_default(
    entry: impl AsRef<Path>,
    source: &str,
    resolver: &dyn FileResolver,
) -> anyhow::Result<String> {
    expand(entry.as_ref(), source, resolver, &ExpandOptions::default())
}

pub fn expand_compressed(
    entry: impl AsRef<Path>,
    source: &str,
    resolver: &dyn FileResolver,
) -> anyhow::Result<String> {
    expand(entry.as_ref(), source, resolver, &ExpandOptions { compress: true })
}
