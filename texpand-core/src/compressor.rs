use tree_sitter::Tree;

/// Compress C/C++ source code while preserving semantic correctness.
///
/// Rules:
/// - Discard comments (AST node kind `"comment"`).
/// - Insert a space between two adjacent identifier-like tokens
///   (last char and first char are both `[a-zA-Z0-9_]`).
/// - Concatenate symbols directly without added spaces.
/// - Before entering and after exiting any `preproc_*` subtree, force `'\n'`.
///   For compound preproc constructs (`preproc_ifdef`, `preproc_if`), also
///   insert `'\n'` before body content so `#ifdef FOO` and its body are
///   separated by a newline.
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

/// Top-level preprocessor *directive* nodes — these open a preprocessing
/// line or block.  Sub-element nodes like `preproc_params` and `preproc_arg`
/// live inside a directive and must *not* trigger a separating newline.
/// Preprocessor *sub-elements* that live inside a directive — these must
/// NOT trigger the enter/exit newline logic.
fn is_preproc_sub_element(kind: &str) -> bool {
    matches!(kind, "preproc_params" | "preproc_arg" | "preproc_defined")
}

/// Top-level preprocessor directive — any `preproc_*` node that is not
/// a sub-element.
fn is_preproc_directive(kind: &str) -> bool {
    kind.starts_with("preproc_") && !is_preproc_sub_element(kind)
}

/// Whether this directive node is a *compound* directive whose body
/// spans multiple logical lines (e.g. `#ifdef … #endif`).
fn is_compound_directive(kind: &str) -> bool {
    kind == "preproc_ifdef" || kind == "preproc_if"
}

fn compress_impl(
    tree: &Tree,
    source: &str,
    mut skip_node: impl FnMut(&tree_sitter::Node, &str) -> bool,
) -> String {
    let mut output = String::new();
    let mut cursor = tree.walk();
    let mut prev_last: Option<char> = None;
    // How many compound preproc blocks we are currently inside of.
    let mut compound_depth: usize = 0;
    // Set when entering a compound directive; cleared after the first
    // `\n` is emitted before body content. Prevents inserting `\n`
    // before every non-leaf sibling in the body.
    let mut need_body_nl: bool = false;

    loop {
        let node = cursor.node();
        let is_leaf = node.child_count() == 0;

        if is_leaf
            && node.kind() != "comment"
            && let Ok(text) = node.utf8_text(source.as_bytes())
            && let Some(ch) = text.chars().next()
        {
            // Any `#` leaf must sit on its own line.
            if ch == '#' && !output.is_empty() && !output.ends_with('\n') {
                output.push('\n');
            }
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
            if is_preproc_directive(node.kind()) && !output.is_empty() && !output.ends_with('\n') {
                output.push('\n');
                prev_last = None;
            }
            if is_compound_directive(node.kind()) {
                compound_depth += 1;
                need_body_nl = true;
            } else if compound_depth > 0 && is_preproc_directive(node.kind()) {
                // `#else` / `#elif` inside a compound block — their
                // bodies also need a leading newline.
                need_body_nl = true;
            }
            continue;
        }

        loop {
            if cursor.goto_next_sibling() {
                // Inside a compound preproc, when we step from the
                // directive header (#ifdef + name) into body content
                // (a non-leaf node), insert exactly one separating
                // newline.
                if compound_depth > 0
                    && need_body_nl
                    && cursor.node().child_count() > 0
                    && !output.ends_with('\n')
                {
                    let t = cursor.node().utf8_text(source.as_bytes()).unwrap_or("");
                    if !t.starts_with('#') {
                        output.push('\n');
                        prev_last = None;
                        need_body_nl = false;
                    }
                }
                break;
            }
            if !cursor.goto_parent() {
                return output;
            }
            if is_preproc_directive(cursor.node().kind()) {
                output.push('\n');
                prev_last = None;
                if is_compound_directive(cursor.node().kind()) {
                    compound_depth = compound_depth.saturating_sub(1);
                }
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
        assert_eq!(result, "int main(){return 0;}");
    }

    #[test]
    fn test_symbol_compaction() {
        let result = compress_source("a + b");
        assert_eq!(result, "a+b");
    }

    #[test]
    fn test_keyword_and_identifier_merged() {
        let result = compress_source("int a = 1;");
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

    #[test]
    fn test_preproc_ifdef_block() {
        let src = "#ifdef FOO\nint x;\n#endif\n";
        let result = compress_source(src);
        assert!(result.contains("#endif"), "should contain #endif");
        assert!(
            result.contains("\n#endif"),
            "#endif must be on its own line: got {result:?}"
        );
        assert!(
            result.contains("#ifdef FOO\n"),
            "#ifdef must end with a newline: got {result:?}"
        );
    }

    #[test]
    fn test_preproc_ifdef_body_separated() {
        let src = "#ifdef FOO\nint x;\n#endif\nint y;";
        let result = compress_source(src);
        assert!(result.starts_with("#ifdef"), "{result:?}");
        assert!(
            result.contains("\n#endif"),
            "#endif must start on own line: got {result:?}"
        );
        assert!(
            !result.starts_with("#ifdef FOOint x;#endif"),
            "should NOT have #endif on same line as body"
        );
    }

    #[test]
    fn test_preproc_ifdef_ifndef_adjacent() {
        let src = "#ifdef A\n#endif\n#ifdef B\n#endif\n";
        let result = compress_source(src);
        assert!(
            result.contains("#endif\n#ifdef"),
            "adjacent blocks separated: got {result:?}"
        );
    }

    #[test]
    fn test_preproc_if_else_endif() {
        let src = "#define FLAG 1\n#if defined(FLAG)\nint x;\n#else\nint y;\n#endif\n";
        let result = compress_source(src);
        assert!(result.contains("\n#if"), "open gets newline");
        assert!(result.contains("\n#else"), "else gets newline");
        assert!(result.contains("\n#endif"), "endif gets newline");
        assert!(
            result.contains("#else\nint y;"),
            "#else body must be on its own line: got {result:?}"
        );
    }

    #[test]
    fn test_define() {
        let src = r#"
#define F(op) \
    Some Actions
func();
        "#;
        let result = compress_source(src);
        assert!(
            result.contains("Actions\nfunc"),
            "macro body and normal statement must be separated by newline: got {result:?}"
        );
    }

    #[test]
    fn test_define_backslash_continuation() {
        let src = "#define A(b, c) \\\n    some \\\n    actions\nint main() {}\n";
        let result = compress_source(src);
        assert!(
            result.starts_with("#define A(b,c)"),
            "macro name and params must not be line-separated: got {result:?}"
        );
        assert!(
            !result.contains("#define A\n"),
            "backslash continuation must not be broken: got {result:?}"
        );
    }

    #[test]
    fn test_ifdef_body_compressed() {
        let src = "#ifdef FOO\nint x;\nint y;\n#endif\n";
        let result = compress_source(src);
        // Body tokens should be compressed, not line-separated.
        assert!(
            result.contains("int x;int y;"),
            "ifdef body should be compressed, not line-separated: got {result:?}"
        );
    }
}
