use anyhow::Result;

/// Trait for abstracting file I/O across platforms.
///
/// Each frontend (CLI, VSCode) implements this to provide file contents
/// from its own storage layer. `texpand-core` never calls `std::fs` directly.
pub trait FileResolver {
    /// Resolve an `#include` path to file content.
    ///
    /// Returns `(canonical_path, source_text)` on success.
    fn resolve_and_read(&self, include_path: &str) -> Result<(String, String)>;
}
