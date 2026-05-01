mod common;
use common::*;

#[test]
fn expand_no_includes() {
    let resolver = FixtureResolver::empty();
    let result = expand_default("main.cpp", "int main() { return 0; }\n", &resolver).unwrap();
    assert_eq!(result, "int main() { return 0; }\n");
}

#[test]
fn expand_single_local_include() {
    let resolver = FixtureResolver::new([("a.h", "int a = 1;\n")]);
    let src = "#include \"a.h\"\nint main() { return a; }\n";
    let result = expand_default("main.cpp", src, &resolver).unwrap();

    assert!(result.contains("int a = 1;"));
    assert!(result.contains("int main() { return a; }"));
    assert!(!result.contains("#include \"a.h\""));
}

#[test]
fn expand_deep_chain() {
    let resolver = FixtureResolver::new([
        ("d.h", "int d = 4;\n"),
        ("c.h", "#include \"d.h\"\nint c = d + 1;\n"),
        ("b.h", "#include \"c.h\"\nint b = c + 1;\n"),
        ("a.h", "#include \"b.h\"\nint a = b + 1;\n"),
    ]);
    let src = "#include \"a.h\"\nint main() { return a; }\n";
    let result = expand_default("main.cpp", src, &resolver).unwrap();

    let pos_d = result.find("int d = 4;").unwrap();
    let pos_c = result.find("int c = d + 1;").unwrap();
    let pos_b = result.find("int b = c + 1;").unwrap();
    let pos_a = result.find("int a = b + 1;").unwrap();
    let pos_main = result.find("int main() { return a; }").unwrap();

    assert!(pos_d < pos_c, "d before c");
    assert!(pos_c < pos_b, "c before b");
    assert!(pos_b < pos_a, "b before a");
    assert!(pos_a < pos_main, "a before main");
}

#[test]
fn expand_diamond_dep() {
    let resolver = FixtureResolver::new([
        ("base.h", "int base = 0;\n"),
        ("left.h", "#include \"base.h\"\nint left = base;\n"),
        ("right.h", "#include \"base.h\"\nint right = base;\n"),
    ]);
    let src = "#include \"left.h\"\n#include \"right.h\"\nint main() {}\n";
    let result = expand_default("main.cpp", src, &resolver).unwrap();

    assert_eq!(
        result.matches("int base = 0;").count(),
        1,
        "common dependency must be expanded only once"
    );

    let pos_base = result.find("int base = 0;").unwrap();
    let pos_left = result.find("int left = base;").unwrap();
    let pos_right = result.find("int right = base;").unwrap();
    assert!(pos_base < pos_left, "base before left");
    assert!(pos_base < pos_right, "base before right");
}

#[test]
fn expand_multiple_includes() {
    let resolver = FixtureResolver::new([
        ("a.h", "int a = 1;\n"),
        ("b.h", "int b = 2;\n"),
        ("c.h", "int c = 3;\n"),
    ]);
    let src = "#include \"a.h\"\n#include \"b.h\"\n#include \"c.h\"\nint main() {}\n";
    let result = expand_default("main.cpp", src, &resolver).unwrap();

    assert!(result.contains("int a = 1;"));
    assert!(result.contains("int b = 2;"));
    assert!(result.contains("int c = 3;"));
    assert!(!result.contains("#include \"a.h\""));
    assert!(!result.contains("#include \"b.h\""));
    assert!(!result.contains("#include \"c.h\""));
}

#[test]
fn pragma_once_in_dependency() {
    let resolver = FixtureResolver::new([("a.h", "#pragma once\nint a = 1;\n")]);
    let src = "#include \"a.h\"\nint main() { return a; }\n";
    let result = expand_default("main.cpp", src, &resolver).unwrap();

    assert!(
        !result.contains("#pragma once"),
        "#pragma once should be stripped from deps"
    );
    assert!(result.contains("int a = 1;"));
}
