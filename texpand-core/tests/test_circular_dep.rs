mod common;
use common::*;

#[test]
fn circular_two_files() {
    let resolver = FixtureResolver::new([
        ("a.h", "#include \"b.h\"\nint a;\n"),
        ("b.h", "#include \"a.h\"\nint b;\n"),
    ]);
    let src = "#include \"a.h\"\nint main() {}\n";
    let err = expand_default("main.cpp", src, &resolver).unwrap_err();
    let msg = format!("{err:#}");
    assert!(
        msg.contains("circular") || msg.contains("cycle"),
        "error should mention circular/cycle: {msg}"
    );
    assert!(
        msg.contains("a.h") && msg.contains("b.h"),
        "error should include cycle participants: {msg}"
    );
}

#[test]
fn circular_three_files() {
    let resolver = FixtureResolver::new([
        ("a.h", "#include \"b.h\"\n"),
        ("b.h", "#include \"c.h\"\n"),
        ("c.h", "#include \"a.h\"\n"),
    ]);
    let src = "#include \"a.h\"\nint main() {}\n";
    let err = expand_default("main.cpp", src, &resolver).unwrap_err();
    let msg = format!("{err:#}");
    assert!(
        msg.contains("circular") || msg.contains("cycle"),
        "error should mention circular/cycle: {msg}"
    );
}
