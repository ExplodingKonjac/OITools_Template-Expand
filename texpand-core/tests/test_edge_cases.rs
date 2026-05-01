mod common;
use common::*;

#[test]
fn empty_file() {
    let resolver = FixtureResolver::empty();
    let result = expand_default("empty.cpp", "", &resolver).unwrap();
    assert_eq!(result, "");
}

#[test]
fn pragma_once_stripped() {
    let resolver = FixtureResolver::empty();
    let result = expand_default("header.h", "#pragma once\nint a = 1;\n", &resolver).unwrap();
    assert!(
        !result.contains("#pragma once"),
        "#pragma once should be stripped"
    );
    assert!(result.contains("int a = 1;"));
}

#[test]
fn file_with_only_includes() {
    let resolver = FixtureResolver::new([("a.h", "int a = 1;\n"), ("b.h", "int b = 2;\n")]);
    let src = "#include \"a.h\"\n#include \"b.h\"\n";
    let result = expand_default("main.cpp", src, &resolver).unwrap();

    assert!(result.contains("int a = 1;"));
    assert!(result.contains("int b = 2;"));
    assert!(!result.contains("#include"));
}

#[test]
fn unresolvable_include() {
    let resolver = FixtureResolver::empty();
    let src = "#include \"nonexistent.h\"\nint main() {}\n";
    let err = expand_default("main.cpp", src, &resolver).unwrap_err();
    assert!(
        format!("{err:#}").contains("nonexistent.h"),
        "error should mention the missing file"
    );
}

#[test]
fn unresolvable_transitive_include() {
    let resolver = FixtureResolver::new([("a.h", "#include \"missing.h\"\nint a;\n")]);
    let src = "#include \"a.h\"\nint main() {}\n";
    let err = expand_default("main.cpp", src, &resolver).unwrap_err();
    assert!(
        format!("{err:#}").contains("missing.h"),
        "error should mention the transitive missing file"
    );
}
