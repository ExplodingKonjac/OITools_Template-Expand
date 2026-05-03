use std::collections::HashSet;

use crate::{
    compressor,
    parser::{Include, classify_include, parse_source},
    resolver::FileResolver,
};

use anyhow::{Result, bail};

/// Options for the expansion.
#[derive(Default)]
pub struct ExpandOptions {
    /// Whether to apply semantic-safe code compression.
    pub compress: bool,
}

// ── Preproc context types ───────────────────────────────────────────────────

/// A token sequence extracted from a preprocessor conditional directive,
/// used for structural equivalence comparison.
type Subject = Vec<String>;

/// A single level in the preprocessor conditional context stack.
#[derive(Debug, Clone, Hash, PartialEq, Eq)]
enum PreprocDirective {
    If(Subject),
    Ifdef(Subject),
    Ifndef(Subject),
    Elif(Subject),
    Elifdef(Subject),
    Else,
}

/// The current preprocessing context at some point in the source — a stack of
/// enclosing conditional directives.
#[derive(Debug, Clone, Hash, PartialEq, Eq, Default)]
struct PreprocContext(Vec<PreprocDirective>);

// ── Subject extraction from tree-sitter nodes ───────────────────────────────

/// Extract the subject token sequence from a preprocessor conditional node.
fn extract_subject(node: &tree_sitter::Node, source: &str) -> Subject {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        let kind = child.kind();

        // Skip the directive keyword (#if, #ifdef, etc.) and preproc_*
        // alternative/body children that are NOT part of the condition.
        match kind {
            "preproc_directive" | "preproc_else" | "preproc_elif" | "preproc_elifdef" => continue,
            _ => {}
        }

        // Also skip any child whose text starts with #
        if let Ok(text) = child.utf8_text(source.as_bytes())
            && (text.trim().starts_with('#') || text.trim().is_empty())
        {
            continue;
        }

        let mut tokens: Subject = Vec::new();
        collect_leaf_tokens(&child, source, &mut tokens);
        if !tokens.is_empty() {
            return tokens;
        }
    }
    Subject::new()
}

/// Collect all non-empty, non-`#` leaf token texts from a subtree.
fn collect_leaf_tokens(node: &tree_sitter::Node, source: &str, tokens: &mut Subject) {
    if node.child_count() == 0 {
        if let Ok(text) = node.utf8_text(source.as_bytes()) {
            let t = text.trim();
            if !t.is_empty() && !t.starts_with('#') {
                tokens.push(t.to_string());
            }
        }
        return;
    }

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        // Recurse into all children of non-leaf nodes
        if child.child_count() > 0 {
            collect_leaf_tokens(&child, source, tokens);
        } else if let Ok(text) = child.utf8_text(source.as_bytes()) {
            let t = text.trim();
            if !t.is_empty() && !t.starts_with('#') {
                tokens.push(t.to_string());
            }
        }
    }
}

// ── ExpandState ─────────────────────────────────────────────────────────────

struct ExpandState<'a> {
    /// Files that have been fully expanded in a given context.
    completed: HashSet<(String, PreprocContext)>,
    /// Files currently on the recursion stack (for cycle detection).
    expanding: HashSet<String>,
    resolver: &'a dyn FileResolver,
}

impl<'a> ExpandState<'a> {
    fn new(resolver: &'a dyn FileResolver) -> Self {
        Self {
            completed: HashSet::new(),
            expanding: HashSet::new(),
            resolver,
        }
    }
}

// ── Public API ──────────────────────────────────────────────────────────────

/// Expand an entry file by recursively inlining its `#include` dependencies.
///
/// Expansion preserves preprocessor conditional structure by tracking the
/// current preprocessor context stack and deduplicating includes that appear
/// under the *same* context.
pub fn expand(
    entry_path: &str,
    entry_source: &str,
    resolver: &dyn FileResolver,
    opts: &ExpandOptions,
) -> Result<String> {
    let mut state = ExpandState::new(resolver);
    let ctx = PreprocContext::default();
    let output = expand_recursive(entry_path, entry_source, &ctx, &mut state)?;

    if opts.compress {
        let tree = parse_source(&output)?;
        Ok(compressor::compress(&tree, &output))
    } else {
        Ok(output)
    }
}

// ── Core recursive expansion ────────────────────────────────────────────────

fn expand_recursive(
    path: &str,
    source: &str,
    parent_context: &PreprocContext,
    state: &mut ExpandState,
) -> Result<String> {
    // Cycle detection
    if !state.expanding.insert(path.to_string()) {
        let mut cycle_participants: Vec<_> = state.expanding.iter().cloned().collect();
        cycle_participants.push(path.to_string());
        bail!(
            "circular dependency detected: {}",
            cycle_participants.join(" -> ")
        );
    }

    let tree = parse_source(source)?;
    let output = walk_file(&tree, source, parent_context, state)?;

    state.expanding.remove(path);
    Ok(output)
}

// ── AST walk: DFS with context stack ────────────────────────────────────────

fn walk_file(
    tree: &tree_sitter::Tree,
    source: &str,
    parent_context: &PreprocContext,
    state: &mut ExpandState,
) -> Result<String> {
    let mut output = String::new();
    let mut cp = parent_context.0.clone();
    let mut byte_pos: usize = 0;
    let mut cursor = tree.walk();

    loop {
        let node = cursor.node();
        let node_start = node.start_byte();
        let node_end = node.end_byte();

        match node.kind() {
            // ── #include ──────────────────────────────────────────────
            "preproc_include" => {
                // Emit everything before this include
                if node_start > byte_pos {
                    output.push_str(&source[byte_pos..node_start]);
                }
                byte_pos = node_end;

                let ctx = PreprocContext(cp.clone());
                match classify_include(&node, source) {
                    Some(Include::Local(path)) => {
                        let key = (path.to_string(), ctx.clone());
                        if !state.completed.contains(&key) {
                            let (resolved_path, content) =
                                state.resolver.resolve_and_read("", path)?;
                            let expanded = expand_recursive(&resolved_path, &content, &ctx, state)?;
                            state.completed.insert(key);
                            output.push_str(&expanded);
                            if !expanded.ends_with('\n') {
                                output.push('\n');
                            }
                        }
                    }
                    Some(Include::System(path)) => {
                        let key = (format!("<{path}>"), ctx);
                        if !state.completed.contains(&key) {
                            state.completed.insert(key);
                            // Emit the include line for first occurrence
                            output.push_str(&source[node_start..node_end]);
                        }
                    }
                    None => {}
                }
            }

            // ── #pragma once — strip entirely (no dedup semantics) ──────────
            "preproc_call" => {
                if let Ok(text) = node.utf8_text(source.as_bytes())
                    && text.trim() == "#pragma once"
                {
                    if node_start > byte_pos {
                        output.push_str(&source[byte_pos..node_start]);
                    }
                    byte_pos = node_end;
                }
            }

            // ── Compound conditional directives (push to context stack) ─
            "preproc_ifdef" => {
                cp.push(PreprocDirective::Ifdef(extract_subject(&node, source)));
            }
            "preproc_ifndef" => {
                cp.push(PreprocDirective::Ifndef(extract_subject(&node, source)));
            }
            "preproc_if" => {
                cp.push(PreprocDirective::If(extract_subject(&node, source)));
            }
            "preproc_else" => {
                cp.push(PreprocDirective::Else);
            }
            "preproc_elif" => {
                cp.push(PreprocDirective::Elif(extract_subject(&node, source)));
            }
            "preproc_elifdef" => {
                cp.push(PreprocDirective::Elifdef(extract_subject(&node, source)));
            }

            // ── Everything else (leaf → emit text) ────────────────────
            _ => {
                if node.child_count() == 0 {
                    // Leaf node — emit original source text
                    if node_start > byte_pos {
                        output.push_str(&source[byte_pos..node_start]);
                    }
                    output.push_str(&source[node_start..node_end]);
                    byte_pos = node_end;
                }
                // Non-leaf: byte_pos stays unchanged; children advance it
            }
        }

        // Enter children (skip nodes we've fully handled at this level)
        if !matches!(node.kind(), "preproc_include" | "preproc_call") && cursor.goto_first_child() {
            continue;
        }

        // Backtrack
        loop {
            if cursor.goto_next_sibling() {
                break;
            }
            if !cursor.goto_parent() {
                // End of traversal — emit remaining source
                if source.len() > byte_pos {
                    output.push_str(&source[byte_pos..]);
                }
                return Ok(output);
            }
            // Exiting a compound directive — pop from context stack
            if matches!(
                cursor.node().kind(),
                "preproc_ifdef"
                    | "preproc_ifndef"
                    | "preproc_if"
                    | "preproc_else"
                    | "preproc_elif"
                    | "preproc_elifdef"
            ) {
                cp.pop();
            }
        }
    }
}

// ── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::resolver::FileResolver;

    struct MockResolver;

    impl FileResolver for MockResolver {
        fn resolve_and_read(&self, _includer: &str, path: &str) -> Result<(String, String)> {
            match path {
                "a.h" => Ok(("a.h".into(), "int a = 1;\n".into())),
                "b.h" => Ok(("b.h".into(), "int b = 2;\n".into())),
                _ => bail!("file not found: {path}"),
            }
        }
    }

    fn expand_default(entry: &str, src: &str, resolver: &dyn FileResolver) -> Result<String> {
        expand(entry, src, resolver, &ExpandOptions::default())
    }

    #[test]
    fn test_no_includes() {
        let src = "int main() { return 0; }\n";
        let result = expand_default("main.cpp", src, &MockResolver).unwrap();
        assert_eq!(result, "int main() { return 0; }\n");
    }

    #[test]
    fn test_single_include() {
        let src = "#include \"a.h\"\nint main() { return a; }\n";
        let result = expand_default("main.cpp", src, &MockResolver).unwrap();
        assert!(result.contains("int a = 1;"));
        assert!(result.contains("int main() { return a; }"));
        assert!(!result.contains("#include \"a.h\""));
    }

    #[test]
    fn test_system_include_preserved() {
        let src = "#include <vector>\n#include \"a.h\"\nint main() { return a; }\n";
        let result = expand_default("main.cpp", src, &MockResolver).unwrap();
        assert!(result.contains("#include <vector>"));
        assert!(!result.contains("#include \"a.h\""));
        assert!(result.contains("int a = 1;"));
    }

    #[test]
    fn test_system_include_in_dependency() {
        let src_a = "#include <string>\nint a = 1;\n";
        let result = expand_default("a.h", src_a, &MockResolver).unwrap();
        assert!(result.contains("#include <string>"));
        assert!(result.contains("int a = 1;"));
    }

    #[test]
    fn test_transitive_order() {
        let src_a = "#include \"b.h\"\nint a = b + 1;\n";
        let result = expand_default("a.h", src_a, &MockResolver).unwrap();
        let pos_b = result.find("int b = 2;").unwrap();
        let pos_a = result.find("int a = b + 1;").unwrap();
        assert!(pos_b < pos_a, "dependencies must come first");
    }

    #[test]
    fn test_pragma_once_stripped() {
        let src_a = "#pragma once\nint a = 1;\n";
        let result = expand_default("a.h", src_a, &MockResolver).unwrap();
        assert!(!result.contains("#pragma once"));
        assert!(result.contains("int a = 1;"));
    }

    #[test]
    fn test_expand_with_compression() {
        let src = "#include \"a.h\"\nint main() { return a; }\n";
        let opts = ExpandOptions { compress: true };
        let result = expand("main.cpp", src, &MockResolver, &opts).unwrap();
        assert!(result.contains("int a=1;"));
        assert!(result.contains("int main(){return a;}"));
        assert!(!result.contains("#include \"a.h\""));
    }

    // ── Conditional compilation context tests ───────────────────────────

    #[test]
    fn test_same_file_same_context_dedup() {
        let src = "#include \"a.h\"\n#include \"a.h\"\n";
        let result = expand_default("main.cpp", src, &MockResolver).unwrap();
        // a.h content should appear only once
        assert_eq!(result.matches("int a = 1;").count(), 1);
    }

    #[test]
    fn test_include_inside_ifdef() {
        let src = "#ifdef USE_FOO\n#include \"a.h\"\n#endif\nint x;\n";
        let result = expand_default("main.cpp", src, &MockResolver).unwrap();
        assert!(result.contains("#ifdef USE_FOO"));
        assert!(result.contains("int a = 1;"));
        assert!(result.contains("int x;"));
    }

    #[test]
    fn test_same_file_different_contexts() {
        let src = "#ifdef X\n#include \"a.h\"\n#else\n#include \"a.h\"\n#endif\n";
        let result = expand_default("main.cpp", src, &MockResolver).unwrap();
        // a.h should appear once in each branch
        assert_eq!(
            result.matches("int a = 1;").count(),
            2,
            "a.h must be expanded in both branches"
        );
        assert!(result.contains("#ifdef X\nint a = 1;\n#else\nint a = 1;\n#endif"));
    }
}
