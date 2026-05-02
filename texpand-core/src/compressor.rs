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

/// How many non-`#` non-leaf siblings appear *before* the body content
/// inside this directive.
///
/// `preproc_if` / `preproc_elif` / `preproc_elifdef`:
///   `#if`, CONDITION(non-leaf), BODY(non-leaf) → 1 (skip condition)
/// `preproc_ifdef` / `preproc_else` / everything else:
///   `#ifdef`, FOO(leaf), BODY(non-leaf) → 0 (first non-leaf IS body)
fn body_siblings_before_body(kind: &str) -> usize {
    match kind {
        "preproc_if" | "preproc_elif" | "preproc_elifdef" => 1,
        _ => 0,
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
    // How many compound preproc blocks we are currently inside of.
    let mut compound_depth: usize = 0;
    // When inside a compound block, counts how many non-`#` non-leaf
    // siblings to skip before the BODY starts. Once body `\n` is emitted,
    // set to `None` to prevent further newlines within this directive.
    let mut body_nl_counter: Option<usize> = None;

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
            }
            // Entering any directive inside a compound block: set up the
            // counter so the body (and only the body) gets a leading `\n`.
            if compound_depth > 0 && is_preproc_directive(node.kind()) {
                body_nl_counter = Some(body_siblings_before_body(node.kind()));
            }
            continue;
        }

        loop {
            if cursor.goto_next_sibling() {
                // Inside a compound preproc, count down non-`#`, non-leaf
                // siblings — when the counter reaches 0, the CURRENT
                // sibling IS the body. Insert exactly one `\n`.
                if compound_depth > 0
                    && let Some(ref mut remaining) = body_nl_counter
                {
                    let n = cursor.node();
                    if n.child_count() > 0 {
                        let t = n.utf8_text(source.as_bytes()).unwrap_or("");
                        if !t.starts_with('#') {
                            if *remaining == 0 && !output.ends_with('\n') {
                                output.push('\n');
                                prev_last = None;
                            }
                            if *remaining == 0 {
                                body_nl_counter = None;
                            } else {
                                *remaining = remaining.saturating_sub(1);
                            }
                        }
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
                body_nl_counter = None;
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
    fn test_defines() {
        let src = r#"
#ifndef _GUARD
#define _GUARD
#endif
#define F(op) \
    Some Actions
func();
        "#;
        let result = compress_source(src);
        eprintln!("{result}");
        assert!(
            result.contains("Actions\nfunc"),
            "macro body and normal statement must be separated by newline: got {result:?}"
        );
        assert!(
            result.contains("#ifndef _GUARD\n#define _GUARD"),
            "#ifndef and #define must be separated: got {result:?}"
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

    #[test]
    fn test_preproc_if_condition() {
        let src = r#"
#if defined(__linux__)
    func1();
#endif

#if 2 > 1
    func2();
#else
    func3();
#endif
        "#;
        let result = compress_source(src);
        eprintln!("{result}");
        assert!(
            result.starts_with("#if defined(__linux__)\nfunc1();\n#endif\n#if 2>1\nfunc2();\n#else\nfunc3();\n#endif"),
            "unexpected output: {result:?}"
        );
    }

    #[test]
    fn test_ifdef_nested() {
        let src = "#ifdef A\n#ifdef B\nint x;\n#endif\n#endif\n";
        let result = compress_source(src);
        assert!(
            result.contains("#ifdef B\n"),
            "inner ifdef on own line: {result:?}"
        );
        assert!(
            result.contains("\n#endif\n#endif"),
            "both endifs separated: {result:?}"
        );
    }

    #[test]
    fn test_include_guard() {
        let src = "#ifndef H\n#define H\nint code;\n#endif\n";
        let result = compress_source(src);
        assert!(
            result.contains("#ifndef H\n#define H"),
            "guard pattern: {result:?}"
        );
    }

    #[test]
    fn test_ifdef_ifdef_block() {
        let src = "#ifdef A\n#ifdef B\nint x;\nint y;\n#endif\n#endif\n";
        let result = compress_source(src);
        assert!(result.contains("int x;int y;"), "body compressed");
    }

    #[test]
    fn test_empty_ifdef_body() {
        let src = "#ifdef A\n#endif\n";
        let result = compress_source(src);
        assert!(
            result.contains("#ifdef A\n#endif"),
            "endif immediately follows"
        );
    }

    #[test]
    fn test_preproc_then_statement() {
        let src = "#include \"h\"\nint x;\n";
        let result = compress_source(src);
        assert!(
            result.contains("\nint x;"),
            "newline between include and code"
        );
    }
}
