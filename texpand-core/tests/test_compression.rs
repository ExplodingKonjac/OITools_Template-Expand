mod common;
use common::*;

#[test]
fn expand_with_compression() {
    let resolver = FixtureResolver::new([("a.h", "int a = 1; // header\n")]);
    let src = "// entry\n#include \"a.h\"\nint main() { return a; }\n";
    let result = expand_compressed("main.cpp", src, &resolver).unwrap();

    assert!(!result.contains("// entry"));
    assert!(!result.contains("// header"));
    assert!(
        result.contains("int a=1;"),
        "code should be compacted: {result}"
    );
    assert!(result.contains("int main(){return a;}"));
    assert!(!result.contains("#include \"a.h\""));
}

#[test]
fn expand_with_compression_system_include() {
    let resolver = FixtureResolver::new([("a.h", "int a = 1;\n")]);
    let src = "#include <cstdio>\n#include \"a.h\"\nint main() { return a; }\n";
    let result = expand_compressed("main.cpp", src, &resolver).unwrap();

    assert!(
        result.contains("#include<cstdio>") || result.contains("#include <cstdio>"),
        "system include must survive compression: {result}"
    );
    assert!(result.contains("int a=1;") || result.contains("int a = 1;"));
}
