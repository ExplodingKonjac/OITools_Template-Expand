/// 将单行的 S-表达式格式化为带缩进的多行字符串
fn pretty_print_sexp(sexp: &str) -> String {
    let mut formatted = String::new();
    let mut indent_level = 0;

    // 遍历每一个字符，简单的根据括号和空格计算缩进
    for c in sexp.chars() {
        match c {
            '(' => {
                indent_level += 1;
                formatted.push(c);
            }
            ')' => {
                indent_level -= 1;
                formatted.push(c);
            }
            ' ' => {
                // 遇到空格时换行，并加上当前的缩进
                formatted.push('\n');
                formatted.push_str(&"  ".repeat(indent_level));
            }
            _ => formatted.push(c),
        }
    }

    formatted
}

pub fn main() {
    use tree_sitter::Parser;
    // 1. 我们想要解析的源代码
    let source_code = r#"
#define FOO bar
"#;

    // 2. 初始化 Parser
    let mut parser = Parser::new();

    // 3. 设置目标语言 (这里设置为 Rust)
    let language = tree_sitter::Language::new(tree_sitter_cpp::LANGUAGE);
    parser
        .set_language(&language)
        .expect("Error loading Rust grammar");

    // 4. 解析源代码，生成 Tree
    let tree = parser.parse(source_code, None).unwrap();

    // 5. 获取 AST 的根节点
    let root_node = tree.root_node();

    // 6. 使用 S-表达式 (S-expression) 格式输出 AST
    println!("源代码:\n{}\n", source_code);
    println!("AST 输出:");
    println!("{}", pretty_print_sexp(&root_node.to_sexp()));
}
