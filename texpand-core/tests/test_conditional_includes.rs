mod common;
use common::*;

#[test]
fn include_inside_ifdef() {
    let resolver = FixtureResolver::new([("foo.h", "int foo_val;\n")]);
    let src = "#ifdef USE_FOO\n#include \"foo.h\"\n#endif\nint x;\n";
    let result = expand_default("main.cpp", src, &resolver).unwrap();

    assert!(result.contains("#ifdef USE_FOO"));
    assert!(result.contains("int foo_val;"));
    assert!(result.contains("int x;"));
    assert!(!result.contains("#include \"foo.h\""));
}

#[test]
fn different_files_in_branches() {
    let resolver = FixtureResolver::new([
        ("a.h", "int a_val;\n"),
        ("b.h", "int b_val;\n"),
    ]);
    let src = "#ifdef A\n#include \"a.h\"\n#else\n#include \"b.h\"\n#endif\n";
    let result = expand_default("main.cpp", src, &resolver).unwrap();

    assert!(result.contains("#ifdef A\nint a_val;\n#else\nint b_val;\n#endif"));
    assert!(!result.contains("#include \"a.h\""));
    assert!(!result.contains("#include \"b.h\""));
}

#[test]
fn same_file_different_contexts() {
    let resolver = FixtureResolver::new([("u.h", "int u;\n")]);
    let src = "#ifdef X\n#include \"u.h\"\n#else\n#include \"u.h\"\n#endif\n";
    let result = expand_default("main.cpp", src, &resolver).unwrap();

    // u.h should appear once in each branch
    assert_eq!(
        result.matches("int u;").count(),
        2,
        "u.h must be expanded in both branches (different contexts)"
    );
    assert!(result.contains("#ifdef X\nint u;\n#else\nint u;\n#endif"));
}

#[test]
fn same_file_same_context_dedup() {
    let resolver = FixtureResolver::new([("u.h", "int u;\n")]);
    let src = "#include \"u.h\"\n#include \"u.h\"\nint x;\n";
    let result = expand_default("main.cpp", src, &resolver).unwrap();

    assert_eq!(
        result.matches("int u;").count(),
        1,
        "u.h should appear only once in same context"
    );
    assert!(!result.contains("#include \"u.h\""));
}

#[test]
fn same_ifdef_twice() {
    let resolver = FixtureResolver::new([("a.h", "int a;\n")]);
    let src = "#ifdef FOO\n#include \"a.h\"\n#endif\n#ifdef FOO\n#include \"a.h\"\n#endif\n";
    let result = expand_default("main.cpp", src, &resolver).unwrap();

    // Same #ifdef FOO → same context → a.h should be expanded once
    assert_eq!(
        result.matches("int a;").count(),
        1,
        "same #ifdef FOO twice must be equivalent contexts"
    );
}

#[test]
fn if_condition_whitespace_normalized() {
    let resolver = FixtureResolver::new([("x.h", "int x;\n")]);
    let src = "#if A == B\n#include \"x.h\"\n#endif\n#if A==B\n#include \"x.h\"\n#endif\n";
    let result = expand_default("main.cpp", src, &resolver).unwrap();

    // Token sequences for both conditions are ["A", "==", "B"] → same context
    assert_eq!(
        result.matches("int x;").count(),
        1,
        "#if A == B and #if A==B must be equivalent (same token seq)"
    );
}

#[test]
fn ifdef_different_identifiers_not_equivalent() {
    let resolver = FixtureResolver::new([("u.h", "int u;\n")]);
    let src = "#ifdef FOO\n#include \"u.h\"\n#endif\n#ifdef BAR\n#include \"u.h\"\n#endif\n";
    let result = expand_default("main.cpp", src, &resolver).unwrap();

    // Different identifiers → different contexts → u.h in both
    assert_eq!(
        result.matches("int u;").count(),
        2,
        "#ifdef FOO vs #ifdef BAR must be different contexts"
    );
}
