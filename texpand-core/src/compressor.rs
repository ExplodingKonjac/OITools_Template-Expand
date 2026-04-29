use tree_sitter::Tree;

/// Compress C/C++ source code while preserving semantic correctness.
///
/// Rules:
/// - Discard comments (AST node kind `"comment"`).
/// - Insert a space between two adjacent identifier-like tokens
///   (last char and first char are both `[a-zA-Z0-9_]`).
/// - Concatenate symbols directly without added spaces.
/// - After any `preproc_*` subtree, force append `'\n'`.
pub fn compress(tree: &Tree, source: &str) -> String {
    compress_impl(tree, source, |_node, _src| false)
}

/// Like `compress`, but also strips `#include` and `#pragma once` lines
/// in a single tree traversal — avoiding a second parse.
///
/// The caller passes the **original (un-stripped)** source and tree; this
/// function skips `preproc_include` and `#pragma once` subtrees entirely
/// (no content emitted, no trailing `\n`).
pub fn compress_stripped(tree: &Tree, source: &str) -> String {
    compress_impl(tree, source, is_strip_node)
}

fn is_strip_node(node: &tree_sitter::Node, source: &str) -> bool {
    match node.kind() {
        "preproc_include" => true,
        "preproc_call" => node
            .utf8_text(source.as_bytes())
            .is_ok_and(|t| t.trim() == "#pragma once"),
        _ => false,
    }
}

fn compress_impl(
    tree: &Tree,
    source: &str,
    mut skip_node: impl FnMut(&tree_sitter::Node, &str) -> bool,
) -> String {
    let mut output = String::new();
    let mut cursor = tree.walk();
    let mut prev_last: Option<char> = None;

    loop {
        let node = cursor.node();
        let is_leaf = node.child_count() == 0;

        if is_leaf
            && node.kind() != "comment"
            && let Ok(text) = node.utf8_text(source.as_bytes())
            && let Some(ch) = text.chars().next()
        {
            if let Some(prev) = prev_last
                && is_ident_char(prev)
                && is_ident_char(ch)
            {
                output.push(' ');
            }
            output.push_str(text);
            prev_last = text.chars().last();
        }

        if !skip_node(&node, source) && cursor.goto_first_child() {
            continue;
        }

        loop {
            if cursor.goto_next_sibling() {
                break;
            }
            if !cursor.goto_parent() {
                return output;
            }
            // After emptying a preproc node's subtree, force a newline.
            if cursor.node().kind().starts_with("preproc_") {
                output.push('\n');
                prev_last = None;
            }
        }
    }
}

fn is_ident_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '_'
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::parse_source;

    fn compress_source(source: &str) -> String {
        let tree = parse_source(source).unwrap();
        compress(&tree, source)
    }

    #[test]
    fn test_comment_removal() {
        let result = compress_source("int a = 1; // comment\n");
        // Symbols compacted, comments removed
        assert_eq!(result, "int a=1;");
    }

    #[test]
    fn test_multi_line_comment_removal() {
        let result = compress_source("/* block\ncomment */ int a;");
        assert_eq!(result, "int a;");
    }

    #[test]
    fn test_identifier_spacing() {
        let result = compress_source("int main() { return 0; }");
        // Spaces preserved between identifiers, compacted elsewhere
        assert_eq!(result, "int main(){return 0;}");
    }

    #[test]
    fn test_symbol_compaction() {
        let result = compress_source("a + b");
        // `+` is not an ident char, so no space needed
        assert_eq!(result, "a+b");
    }

    #[test]
    fn test_keyword_and_identifier_merged() {
        let result = compress_source("int a = 1;");
        // "int a" needs space (ident→ident), "a = 1" compacted
        assert_eq!(result, "int a=1;");
    }

    #[test]
    fn test_preproc_newline() {
        let result = compress_source("#include \"foo.h\"\nint a;");
        assert!(
            result.contains("\nint a;") || result.ends_with("\nint a;"),
            "preproc should end with \\n: got {result:?}"
        );
    }

    #[test]
    fn test_multiple_preproc() {
        let src = "#include \"a.h\"\n#include \"b.h\"\nint x;";
        let result = compress_source(src);
        assert!(
            result.contains("\n#include\"b.h\"\n"),
            "each preproc gets \\n"
        );
    }

    #[test]
    fn test_define_preserves_newline() {
        let result = compress_source("#define FOO 42\nint a;");
        assert!(
            result.contains("\nint a;"),
            "define must end with newline: got {result:?}"
        );
    }

    #[test]
    fn test_empty_source() {
        assert_eq!(compress_source(""), "");
    }

    #[test]
    fn test_only_comment() {
        assert_eq!(compress_source("// just a comment\n"), "");
    }
}
