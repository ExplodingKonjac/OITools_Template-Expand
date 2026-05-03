use anyhow::{Context, Result};
use tree_sitter::{Node, Parser, Tree};

/// Parse C/C++ source text into a tree-sitter syntax tree.
pub fn parse_source(source: &str) -> Result<Tree> {
    let mut parser = Parser::new();
    parser
        .set_language(&tree_sitter::Language::new(tree_sitter_cpp::LANGUAGE))
        .context("failed to set C++ language")?;

    parser.parse(source, None).context("failed to parse source")
}

/// Represents a `#include` directive found in source code.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Include<'a> {
    /// `#include "..."` — local header that can be resolved by `FileResolver`.
    Local(&'a str),
    /// `#include <...>` — system header, kept as-is in output.
    System(&'a str),
}

/// Extract all `#include` paths (both local and system) from a parsed syntax tree.
pub fn extract_all_includes<'a>(tree: &Tree, source: &'a str) -> Vec<Include<'a>> {
    let mut result = Vec::new();
    let mut cursor = tree.walk();

    loop {
        let node = cursor.node();

        if node.kind() == "preproc_include"
            && let Some(inc) = classify_include(&node, source)
        {
            result.push(inc);
        }

        if cursor.goto_first_child() {
            continue;
        }

        loop {
            if cursor.goto_next_sibling() {
                break;
            }
            if !cursor.goto_parent() {
                return result;
            }
        }
    }
}

/// Extract only local (`#include "..."`) paths for resolution.
pub fn extract_include_paths<'a>(tree: &Tree, source: &'a str) -> Vec<&'a str> {
    extract_all_includes(tree, source)
        .into_iter()
        .filter_map(|inc| match inc {
            Include::Local(p) => Some(p),
            Include::System(_) => None,
        })
        .collect()
}

pub fn classify_include<'a>(node: &Node, source: &'a str) -> Option<Include<'a>> {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        match child.kind() {
            "string_literal" => {
                let raw = child.utf8_text(source.as_bytes()).ok()?;
                let path = raw.get(1..raw.len().checked_sub(1)?)?;
                return Some(Include::Local(path));
            }
            "system_lib_string" => {
                let raw = child.utf8_text(source.as_bytes()).ok()?;
                let path = raw.get(1..raw.len().checked_sub(1)?)?;
                return Some(Include::System(path));
            }
            _ => continue,
        }
    }
    None
}

/// Check whether a `preproc_include` node is a quoted local include (`#include "..."`).
pub fn is_quoted_include(node: &Node, source: &str) -> bool {
    matches!(classify_include(node, source), Some(Include::Local(_)))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn extract(source: &str) -> Vec<String> {
        let tree = parse_source(source).unwrap();
        extract_include_paths(&tree, source)
            .into_iter()
            .map(|s| s.to_string())
            .collect()
    }

    #[test]
    fn test_quoted_include() {
        assert_eq!(extract("#include \"myheader.h\"\n"), vec!["myheader.h"]);
    }

    #[test]
    fn test_system_include_ignored() {
        assert!(extract("#include <vector>\n").is_empty());
    }

    #[test]
    fn test_mixed_includes() {
        let source = "#include \"config.h\"\n#include <cstdio>\n#include \"utils.h\"\n";
        assert_eq!(extract(source), vec!["config.h", "utils.h"]);
    }

    #[test]
    fn test_no_includes() {
        assert!(extract("int main() { return 0; }\n").is_empty());
    }

    #[test]
    fn test_extract_all_includes() {
        let source = "#include \"local.h\"\n#include <system>\n";
        let tree = parse_source(source).unwrap();
        let all = extract_all_includes(&tree, source);
        assert_eq!(all.len(), 2);
        assert_eq!(all[0], Include::Local("local.h"));
        assert_eq!(all[1], Include::System("system"));
    }
}
