use std::{
    collections::{HashMap, HashSet},
    path::{Path, PathBuf},
};

use crate::{
    compressor::{self, CompressorState},
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

        match kind {
            "preproc_directive" | "preproc_else" | "preproc_elif" | "preproc_elifdef" => continue,
            _ => {}
        }

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
    completed: HashSet<(PathBuf, PreprocContext)>,
    /// Files currently on the recursion stack (for cycle detection).
    expanding: HashSet<PathBuf>,
    /// Parsed tree-sitter trees cached by file path.
    tree_cache: HashMap<PathBuf, tree_sitter::Tree>,
    resolver: &'a dyn FileResolver,
}

impl<'a> ExpandState<'a> {
    fn new(resolver: &'a dyn FileResolver) -> Self {
        Self {
            completed: HashSet::new(),
            expanding: HashSet::new(),
            tree_cache: HashMap::new(),
            resolver,
        }
    }
}

// ── Public API ──────────────────────────────────────────────────────────────

/// Expand an entry file by recursively inlining its `#include` dependencies.
pub fn expand(
    entry_path: &Path,
    entry_source: &str,
    resolver: &dyn FileResolver,
    opts: &ExpandOptions,
) -> Result<String> {
    let mut state = ExpandState::new(resolver);
    let ctx = PreprocContext::default();
    expand_recursive(entry_path, entry_source, &ctx, &mut state, opts.compress)
}

// ── Core recursive expansion ────────────────────────────────────────────────

fn expand_recursive(
    path: &Path,
    source: &str,
    parent_context: &PreprocContext,
    state: &mut ExpandState,
    compress: bool,
) -> Result<String> {
    // Cycle detection
    if !state.expanding.insert(path.to_owned()) {
        let mut cycle_participants: Vec<_> = state.expanding.iter().cloned().collect();
        cycle_participants.push(path.to_owned());
        bail!(
            "circular dependency detected: {}",
            cycle_participants
                .iter()
                .map(|p| format!("{}", p.display()))
                .collect::<Vec<_>>()
                .join(" -> ")
        );
    }

    // AST cache: avoid re-parsing the same file more than once
    let tree = if let Some(cached) = state.tree_cache.get(path) {
        cached.clone()
    } else {
        let tree = parse_source(source)?;
        state.tree_cache.insert(path.to_owned(), tree.clone());
        tree
    };
    let mut output = String::with_capacity(source.len());
    let mut byte_pos = 0;
    let mut cs = compress.then(|| CompressorState::new(source.len() / 2));
    let mut cp = parent_context.0.clone();
    let mut saved_depths = Vec::new();
    let mut cursor = tree.walk();

    // ── DFS walk ──────────────────────────────────────────────────────────

    loop {
        let node = cursor.node();
        let node_start = node.start_byte();
        let node_end = node.end_byte();

        let mut skip_children = false;

        match node.kind() {
            // ── #include ──────────────────────────────────────────────
            "preproc_include" => {
                skip_children = true;
                // One gap emission call — only needed in uncompressed mode.
                if !compress && node_start > byte_pos {
                    output.push_str(&source[byte_pos..node_start]);
                }

                // When a preproc_include node is the body of a compound
                // directive, the compressor inserts `\n` via enter_preproc
                // before entering children. Since we skip children of
                // include nodes, simulate that newline here.
                if let Some(ref mut cs) = cs {
                    cs.ensure_newline();
                }

                let ctx = PreprocContext(cp.clone());
                match classify_include(&node, source) {
                    Some(Include::Local(inc_path)) => {
                        let inc_path = state.resolver.resolve(path, inc_path)?;
                        let key = (inc_path.clone(), ctx.clone());
                        if !state.completed.contains(&key) {
                            let content = state.resolver.read_content(&inc_path)?;
                            let expanded =
                                expand_recursive(&inc_path, &content, &ctx, state, compress)?;

                            let out = match cs.as_mut() {
                                Some(cs) => &mut cs.output,
                                None => &mut output,
                            };
                            out.push_str(&expanded);
                            if !expanded.ends_with('\n') {
                                out.push('\n');
                            }
                            state.completed.insert(key);
                        }
                    }
                    Some(Include::System(path)) => {
                        let key = (format!("<{path}>").into(), ctx);
                        if !state.completed.contains(&key) {
                            if let Some(cs) = cs.as_mut() {
                                if let Ok(text) = node.utf8_text(source.as_bytes()) {
                                    cs.emit_token(text, false);
                                }
                            } else {
                                output.push_str(&source[node_start..node_end]);
                            }
                            state.completed.insert(key);
                        }
                    }
                    None => {}
                }

                // Advance past the include line — only needed in uncompressed
                // mode (compressed mode never emits it for local includes,
                // and handles system includes in the seen-or-not branch above).
                if !compress {
                    byte_pos = node_end;
                }
            }

            // ── #pragma once — strip entirely ─────────────────────────
            "preproc_call" => {
                if node
                    .utf8_text(source.as_bytes())
                    .is_ok_and(|t| t.trim() == "#pragma once")
                {
                    skip_children = true;
                    if !compress && node_end > byte_pos {
                        byte_pos = node_end;
                    }
                }
                // Other preproc_call nodes (#define, #undef, etc.)
                // fall through — children are entered naturally and
                // leaf texts emitted by `_ =>`.
            }

            // ── Compound conditional directives ───────────────────────
            "preproc_ifdef" => {
                cp.push(PreprocDirective::Ifdef(extract_subject(&node, source)));
                saved_depths.push(cp.len());
            }
            "preproc_ifndef" => {
                cp.push(PreprocDirective::Ifndef(extract_subject(&node, source)));
                saved_depths.push(cp.len());
            }
            "preproc_if" => {
                cp.push(PreprocDirective::If(extract_subject(&node, source)));
                saved_depths.push(cp.len());
            }

            // ── Alternative branches (just push; parent stays on stack) ─
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
                if node.child_count() == 0
                    && let Some(ref mut cs) = cs
                {
                    if node.kind() != "comment"
                        && let Ok(text) = node.utf8_text(source.as_bytes())
                    {
                        cs.emit_token(text, false);
                    }
                    if node_start > byte_pos {
                        output.push_str(&source[byte_pos..node_start]);
                    }
                    output.push_str(&source[node_start..node_end]);
                    byte_pos = node_end;
                }
            }
        }

        // Enter children (skip nodes we've fully handled at this level)
        if !skip_children && cursor.goto_first_child() {
            if let Some(ref mut cs) = cs
                && compressor::is_preproc_directive(node.kind())
            {
                cs.enter_preproc_directive();
                if compressor::is_compound_directive(node.kind()) {
                    cs.enter_compound_directive(node.kind());
                } else {
                    cs.enter_preproc_child(node.kind());
                }
            }
            continue;
        }

        // Backtrack
        loop {
            if cursor.goto_next_sibling() {
                if let Some(ref mut cs) = cs {
                    cs.on_next_sibling(&cursor.node(), source);
                }
                break;
            }
            if !cursor.goto_parent() {
                state.expanding.remove(path);
                let result = if compress {
                    cs.unwrap().finish()
                } else {
                    if source.len() > byte_pos {
                        output.push_str(&source[byte_pos..]);
                    }
                    output
                };
                return Ok(result);
            }
            // Exiting a compound directive — truncate everything added
            // inside this block (#ifdef + any #else / #elif branches).
            let parent_kind = cursor.node().kind();
            if matches!(
                parent_kind,
                "preproc_ifdef" | "preproc_ifndef" | "preproc_if"
            ) && let Some(saved) = saved_depths.pop()
            {
                cp.truncate(saved - 1);
            }
            if let Some(ref mut cs) = cs
                && compressor::is_preproc_directive(parent_kind)
            {
                cs.exit_preproc_directive(parent_kind);
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
        fn resolve(&self, _includer_path: &Path, include_path: &str) -> Result<PathBuf> {
            match include_path {
                "a.h" => Ok("a.h".into()),
                "b.h" => Ok("b.h".into()),
                _ => bail!("file not found: {include_path}"),
            }
        }
        fn read_content(&self, resolved_path: &Path) -> Result<String> {
            match resolved_path.to_string_lossy().as_ref() {
                "a.h" => Ok("int a = 1;\n".into()),
                "b.h" => Ok("int b = 2;\n".into()),
                _ => bail!("file not found: {}", resolved_path.display()),
            }
        }
    }

    fn expand_default(
        entry: impl AsRef<Path>,
        src: &str,
        resolver: &dyn FileResolver,
    ) -> Result<String> {
        expand(entry.as_ref(), src, resolver, &ExpandOptions::default())
    }

    fn expand_compressed(
        entry: impl AsRef<Path>,
        src: &str,
        resolver: &dyn FileResolver,
    ) -> Result<String> {
        expand(
            entry.as_ref(),
            src,
            resolver,
            &ExpandOptions { compress: true },
        )
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
        let result = expand_compressed("main.cpp", src, &MockResolver).unwrap();
        assert!(result.contains("int a=1;"));
        assert!(result.contains("int main(){return a;}"));
        assert!(!result.contains("#include \"a.h\""));
    }

    // ── Conditional compilation context tests ───────────────────────────

    #[test]
    fn test_same_file_same_context_dedup() {
        let src = "#include \"a.h\"\n#include \"a.h\"\n";
        let result = expand_default("main.cpp", src, &MockResolver).unwrap();
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
        assert_eq!(
            result.matches("int a = 1;").count(),
            2,
            "a.h must be expanded in both branches"
        );
        assert!(result.contains("#ifdef X\nint a = 1;\n#else\nint a = 1;\n#endif"));
    }

    // ── Compression integration tests ───────────────────────────────────

    #[test]
    fn test_compressed_no_includes() {
        let src = "int main() { return 0; }\n";
        let result = expand_compressed("main.cpp", src, &MockResolver).unwrap();
        assert_eq!(result, "int main(){return 0;}");
    }

    #[test]
    fn test_compressed_with_local_include() {
        let src = "#include \"a.h\"\nint main() { return a; }\n";
        let result = expand_compressed("main.cpp", src, &MockResolver).unwrap();
        assert!(result.contains("int a=1;"));
        assert!(result.contains("int main(){return a;}"));
        assert!(!result.contains("#include \"a.h\""));
    }

    #[test]
    fn test_compressed_system_include() {
        let src = "#include <vector>\n#include \"a.h\"\nint main() { return a; }\n";
        let result = expand_compressed("main.cpp", src, &MockResolver).unwrap();
        assert!(result.contains("#include<vector>") || result.contains("#include <vector>"));
        assert!(result.contains("int a=1;"));
    }

    #[test]
    fn test_compressed_ifdef_include() {
        let src = "#ifdef USE_FOO\n#include \"a.h\"\n#endif\nint x;\n";
        let result = expand_compressed("main.cpp", src, &MockResolver).unwrap();
        assert!(result.contains("#ifdef USE_FOO\nint a=1;"));
        assert!(result.contains("int x;"));
    }
}
