mod common;
use common::*;

#[test]
fn system_include_preserved() {
    let resolver = FixtureResolver::new([("a.h", "int a = 1;\n")]);
    let src = "#include <vector>\n#include \"a.h\"\nint main() { return a; }\n";
    let result = expand_default("main.cpp", src, &resolver).unwrap();

    assert!(
        result.contains("#include <vector>"),
        "system include must survive"
    );
    assert!(result.contains("int a = 1;"));
    assert!(!result.contains("#include \"a.h\""));
}

#[test]
fn system_include_in_dep() {
    let resolver = FixtureResolver::new([("a.h", "#include <string>\nint a = 1;\n")]);
    let src = "#include \"a.h\"\nint main() { return a; }\n";
    let result = expand_default("main.cpp", src, &resolver).unwrap();

    assert!(
        result.contains("#include <string>"),
        "system include in dependency must survive"
    );
    assert!(result.contains("int a = 1;"));
}
