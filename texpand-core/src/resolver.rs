use std::path::{Path, PathBuf};

use anyhow::Result;

/// Trait for abstracting file I/O across platforms.
///
/// Each frontend (CLI, VSCode) implements this to provide file contents
/// from its own storage layer. `texpand-core` never calls `std::fs` directly.
pub trait FileResolver {
    /// Resolve an `#include` path.
    ///
    /// * `includer_path` — the file that contains the `#include` directive
    ///   (useful for resolving relative includes).
    /// * `include_path` — the path as written in the `#include` directive.
    ///
    /// Returns `canonical_path` on success.
    fn resolve(&self, includer_path: &Path, include_path: &str) -> Result<PathBuf>;

    /// Read content from a resolved path
    ///
    /// * `resolved_path` — the resolved path of a included file.
    ///
    /// Returns the content of the file on success.
    fn read_content(&self, resolved_path: &Path) -> Result<String>;
}
