use std::collections::{HashMap, HashSet, VecDeque};

use anyhow::Result;

use crate::compressor;
use crate::graph::DependencyGraph;
use crate::parser::{Include, extract_all_includes, is_quoted_include, parse_source};
use crate::resolver::FileResolver;

/// Options for the expansion.
#[derive(Default)]
pub struct ExpandOptions {
    /// Whether to apply semantic-safe code compression.
    pub compress: bool,
}

/// Expand an entry file by resolving and concatenating its `#include` dependencies.
///
/// * `entry_path` — identifier for the entry file (used as graph node label).
/// * `entry_source` — source text of the entry file.
/// * `resolver` — platform-specific file resolver (CLI or VSCode).
/// * `opts` — expansion options (compression, etc.).
pub fn expand(
    entry_path: &str,
    entry_source: &str,
    resolver: &dyn FileResolver,
    opts: &ExpandOptions,
) -> Result<String> {
    let mut graph = DependencyGraph::new();
    let mut files: HashMap<String, String> = HashMap::new();
    let mut cleaned: HashMap<String, String> = HashMap::new();
    let mut system_headers: HashSet<String> = HashSet::new();

    graph.add_file(entry_path);
    files.insert(entry_path.to_string(), entry_source.to_string());

    // BFS: discover all included files
    let mut queue: VecDeque<String> = VecDeque::new();
    queue.push_back(entry_path.to_string());

    while let Some(path) = queue.pop_front() {
        // Clone to avoid borrow conflicts when inserting into `files`.
        let source = files.get(&path).expect("file source should exist").clone();
        let tree = parse_source(&source)?;

        // Classify and handle includes
        for inc in extract_all_includes(&tree, &source) {
            match inc {
                Include::Local(include_path) => {
                    graph.add_dependency(&path, include_path);
                    let (resolved_path, content) = resolver.resolve_and_read(include_path)?;

                    if !files.contains_key(&resolved_path) {
                        files.insert(resolved_path.clone(), content);
                        queue.push_back(resolved_path);
                    }
                }
                Include::System(system_path) => {
                    graph.add_dependency(&path, system_path);
                    system_headers.insert(system_path.to_string());
                }
            }
        }

        // Strip only local `#include "..."` lines from this file
        let stripped = strip_local_includes(&tree, &source);
        cleaned.insert(
            path,
            if opts.compress {
                compress_source(&stripped)
            } else {
                stripped
            },
        );
    }

    let order = graph.expansion_order()?;

    // Concatenate: skip system header nodes (their include lines are
    // preserved inside the parent files and not stripped).
    let mut output = String::new();
    for path in &order {
        if system_headers.contains(path) {
            continue;
        }
        if let Some(source) = cleaned.get(path) {
            output.push_str(source);
            if !source.ends_with('\n') {
                output.push('\n');
            }
        }
    }

    Ok(output)
}

/// Remove `#include "..."` lines (quoted local includes) from source.
/// System includes (`#include <...>`) are preserved.
fn strip_local_includes(tree: &tree_sitter::Tree, source: &str) -> String {
    let mut cursor = tree.walk();
    let mut ranges: Vec<std::ops::Range<usize>> = Vec::new();

    loop {
        let node = cursor.node();
        if node.kind() == "preproc_include" && is_quoted_include(&node, source) {
            ranges.push(node.start_byte()..node.end_byte());
        }

        if cursor.goto_first_child() {
            continue;
        }

        loop {
            if cursor.goto_next_sibling() {
                break;
            }
            if !cursor.goto_parent() {
                ranges.sort_by(|a, b| b.start.cmp(&a.start));
                let mut result = source.to_string();
                for range in &ranges {
                    result.replace_range(range.clone(), "");
                }
                return result;
            }
        }
    }
}

fn compress_source(source: &str) -> String {
    parse_source(source)
        .map(|tree| compressor::compress(&tree, source))
        .unwrap_or_else(|_| source.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::resolver::FileResolver;

    struct MockResolver;

    impl FileResolver for MockResolver {
        fn resolve_and_read(&self, path: &str) -> Result<(String, String)> {
            match path {
                "a.h" => Ok(("a.h".into(), "int a = 1;\n".into())),
                "b.h" => Ok(("b.h".into(), "int b = 2;\n".into())),
                _ => anyhow::bail!("file not found: {path}"),
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
        // Local include line must be stripped
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
    fn test_transitive_order() {
        let src_a = "#include \"b.h\"\nint a = b + 1;\n";
        let result = expand_default("a.h", src_a, &MockResolver).unwrap();
        let pos_b = result.find("int b = 2;").unwrap();
        let pos_a = result.find("int a = b + 1;").unwrap();
        assert!(pos_b < pos_a, "dependencies must come first");
    }

    #[test]
    fn test_system_include_in_dependency() {
        // a.h includes <string> as a system header
        let src_a = "#include <string>\nint a = 1;\n";
        let result = expand_default("a.h", src_a, &MockResolver).unwrap();
        // <string> should be preserved in a.h's expanded output
        assert!(result.contains("#include <string>"));
        assert!(result.contains("int a = 1;"));
    }

    #[test]
    fn test_expand_with_compression() {
        let src = "#include \"a.h\"\nint main() { return a; }\n";
        let opts = ExpandOptions { compress: true };
        let result = expand("main.cpp", src, &MockResolver, &opts).unwrap();
        // Comments from entry are stripped (none here)
        assert!(result.contains("int a=1;"));
        assert!(result.contains("int main(){return a;}"));
        // Local include line must be stripped
        assert!(!result.contains("#include \"a.h\""));
    }
}
