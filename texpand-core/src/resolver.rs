use anyhow::Result;

/// Trait for abstracting file I/O across platforms.
///
/// Each frontend (CLI, VSCode) implements this to provide file contents
/// from its own storage layer. `texpand-core` never calls `std::fs` directly.
pub trait FileResolver {
    /// Resolve an `#include` path to file content.
    ///
    /// * `includer_path` — the file that contains the `#include` directive
    ///   (useful for resolving relative includes).
    /// * `include_path` — the path as written in the `#include` directive.
    ///
    /// Returns `(canonical_path, source_text)` on success.
    fn resolve_and_read(&self, includer_path: &str, include_path: &str)
    -> Result<(String, String)>;
}
