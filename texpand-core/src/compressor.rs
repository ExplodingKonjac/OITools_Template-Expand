use tree_sitter::Tree;

// ── CompressorState: reusable compression state machine ─────────────────────

/// Reusable state machine for compressing C/C++ token output during a
/// tree-sitter tree walk.
pub struct CompressorState {
    pub output: String,
    prev_last: Option<char>,
    compound_depth: usize,
    body_nl_counter: Option<usize>,
}

impl CompressorState {
    pub fn new(capacity: usize) -> Self {
        Self {
            output: String::with_capacity(capacity),
            prev_last: None,
            compound_depth: 0,
            body_nl_counter: None,
        }
    }

    /// Ensure the output ends with a newline (used when a skipped preproc
    /// node would have triggered `enter_preproc_directive` in the compressor).
    pub fn ensure_newline(&mut self) {
        if !self.output.is_empty() && !self.output.ends_with('\n') {
            self.output.push('\n');
            self.prev_last = None;
        }
    }

    /// Ensure the output ends with a space (used after `#define` macro
    /// names to separate them from the replacement text).
    pub fn ensure_trailing_space(&mut self) {
        if self.prev_last != Some(' ') {
            self.output.push(' ');
            self.prev_last = Some(' ');
        }
    }

    /// Emit a single token, applying identifier-spacing and `#`-newline rules.
    ///
    /// Set `skip_space_before` to `true` to suppress the identifier-space
    /// insertion for the current token (used for `literal_suffix` inside a
    /// `user_defined_literal` node, where `123_km` must remain `123_km`).
    pub fn emit_token(&mut self, text: &str, skip_space_before: bool) {
        if let Some(ch) = text.chars().next() {
            // Any `#` leaf must sit on its own line.
            if ch == '#' && !self.output.is_empty() && !self.output.ends_with('\n') {
                self.output.push('\n');
            }
            if !skip_space_before
                && let Some(prev) = self.prev_last
                && is_ident_char(prev)
                && is_ident_char(ch)
            {
                self.output.push(' ');
            }
            self.output.push_str(text);
            self.prev_last = text.chars().last();
        }
    }

    /// Called before entering any preproc directive node.
    pub fn enter_preproc_directive(&mut self) {
        if !self.output.is_empty() && !self.output.ends_with('\n') {
            self.output.push('\n');
            self.prev_last = None;
        }
    }

    /// Called when entering a compound preproc directive (#ifdef, #if).
    pub fn enter_compound_directive(&mut self, kind: &str) {
        self.compound_depth += 1;
        self.body_nl_counter = Some(body_siblings_before_body(kind));
    }

    /// Called after `goto_next_sibling` inside a compound preproc to detect
    /// when the walker reaches the body.
    pub fn on_next_sibling(&mut self, node: &tree_sitter::Node, source: &str) {
        if self.compound_depth > 0
            && let Some(ref mut remaining) = self.body_nl_counter
            && node.child_count() > 0
        {
            let t = node.utf8_text(source.as_bytes()).unwrap_or("");
            if !t.starts_with('#') {
                if *remaining == 0 && !self.output.ends_with('\n') {
                    self.output.push('\n');
                    self.prev_last = None;
                }
                if *remaining == 0 {
                    self.body_nl_counter = None;
                } else {
                    *remaining = remaining.saturating_sub(1);
                }
            }
        }
    }

    /// Called when exiting a preproc directive (via goto_parent).
    pub fn exit_preproc_directive(&mut self, kind: &str) {
        self.output.push('\n');
        self.prev_last = None;
        if is_compound_directive(kind) {
            self.compound_depth = self.compound_depth.saturating_sub(1);
        }
        self.body_nl_counter = None;
    }

    /// Called after `enter_preproc_directive` for any preproc directive
    /// that lives inside a compound block (including `#else`, `#elif`).
    /// Sets up the body newline counter so `on_next_sibling` can insert
    /// `\n` before the body.
    pub fn enter_preproc_child(&mut self, kind: &str) {
        if self.compound_depth > 0 {
            self.body_nl_counter = Some(body_siblings_before_body(kind));
        }
    }

    /// Consume and return the compressed output.
    pub fn finish(self) -> String {
        self.output
    }
}

// ── Public API ──────────────────────────────────────────────────────────────

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

// ── Private implementation ──────────────────────────────────────────────────

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
fn is_preproc_sub_element(kind: &str) -> bool {
    matches!(kind, "preproc_params" | "preproc_arg" | "preproc_defined")
}

/// Top-level preprocessor directive — any `preproc_*` node that is not
/// a sub-element.
pub(crate) fn is_preproc_directive(kind: &str) -> bool {
    kind.starts_with("preproc_") && !is_preproc_sub_element(kind)
}

/// Whether this directive node is a *compound* directive whose body
/// spans multiple logical lines (e.g. `#ifdef … #endif`).
pub(crate) fn is_compound_directive(kind: &str) -> bool {
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
    let mut st = CompressorState::new(source.len() / 2);
    let mut cursor = tree.walk();

    loop {
        let node = cursor.node();
        let is_leaf = node.child_count() == 0;

        if is_leaf
            && node.kind() != "comment"
            && let Ok(text) = node.utf8_text(source.as_bytes())
            && !text.is_empty()
        {
            st.emit_token(text, node.kind() == "literal_suffix");
            // `#define FOO …` — ensure at least one space between the
            // macro name and the replacement text so that
            //   #define FOO"abc"   and   #define FOO(expr)
            // don't become syntactically wrong.
            if cursor.field_name() == Some("name")
                && let Some(parent) = node.parent()
                && parent.kind() == "preproc_def"
            {
                st.ensure_trailing_space();
            }
        }

        if !skip_node(&node, source) && cursor.goto_first_child() {
            if is_preproc_directive(node.kind()) {
                st.enter_preproc_directive();
            }
            if is_compound_directive(node.kind()) {
                st.enter_compound_directive(node.kind());
            }
            if st.compound_depth > 0 && is_preproc_directive(node.kind()) {
                st.body_nl_counter = Some(body_siblings_before_body(node.kind()));
            }
            continue;
        }

        loop {
            if cursor.goto_next_sibling() {
                st.on_next_sibling(&cursor.node(), source);
                break;
            }
            if !cursor.goto_parent() {
                return st.finish();
            }
            if is_preproc_directive(cursor.node().kind()) {
                st.exit_preproc_directive(cursor.node().kind());
            }
        }
    }
}

pub(crate) fn is_ident_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '_'
}

// ── Tests ───────────────────────────────────────────────────────────────────

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

    #[test]
    fn test_define_space_after_name() {
        // Object-like macros: space between name and replacement
        let s = compress_source("#define FOO BAR\n");
        assert!(s.contains("FOO BAR"), "#define FOO BAR: {s:?}");
        let s = compress_source("#define FOO\"abc\"\n");
        assert!(s.contains("FOO \"abc\""), "#define FOO\"abc\": {s:?}");
        let s = compress_source("#define FOO (X)\n");
        assert!(s.contains("FOO (X)"), "#define FOO (X): {s:?}");
    }

    #[test]
    fn test_define_function_like_no_false_positive() {
        // Function-like macros: `(` must stay immediately after the name.
        let s = compress_source("#define FOO(x) ((x)+(x))\n");
        assert!(
            !s.contains("FOO (x)"),
            "function-like must not get a space: {s:?}"
        );
        assert!(s.contains("FOO(x)"), "function-like FOO(x): {s:?}");
    }

    #[test]
    fn test_user_defined_literal_no_space() {
        // User-defined literals must not get a space between the literal
        // and its suffix: `123_km` not `123 _km`.
        let s = compress_source("auto x = 123_km;\n");
        assert!(s.contains("123_km"), "user-defined literal broken: {s:?}");
        let s = compress_source("auto y = 1.5_deg;\n");
        assert!(
            s.contains("1.5_deg"),
            "user-defined float literal broken: {s:?}"
        );
        let s = compress_source("auto y = 114min;\n");
        assert!(
            s.contains("114min"),
            "literal without '_' prefix broken: {s:?}"
        );
    }
}
