//! A qualified type names its scope; the scope itself is not a type.
use rust_joern::{analyze, graph::Options};

fn options() -> Options {
    Options {
        language: Some("cpp".into()),
        preprocessed: true,
        strict: true,
        ..Options::default()
    }
}

#[test]
fn statements_after_qualified_types_stay_expressions() {
    for source in [
        "void f(std::string path) {\n  std::cout << \"x\" << path;\n}\n",
        "void f(int p) {\n  foo::bar q;\n  foo::baz + p;\n}\n",
        "void f(foo::bar p) {\n  foo::baz << 1;\n}\n",
    ] {
        let analysis = analyze(source, "t.cpp", &options())
            .unwrap_or_else(|error| panic!("{source}: {error}"));
        let calls = analysis.functions[0]
            .cpg
            .nodes
            .iter()
            .filter(|node| node.kind == "CALL")
            .count();
        assert!(calls >= 1, "{source}");
    }
}

#[test]
fn qualified_declarations_remain_declarations() {
    let analysis = analyze(
        "void f(int p) {\n  foo::bar q;\n  foo::bar r;\n  foo::bar *s = 0;\n}\n",
        "t.cpp",
        &options(),
    )
    .unwrap();
    let locals = analysis.functions[0]
        .cpg
        .nodes
        .iter()
        .filter(|node| node.kind == "LOCAL")
        .count();
    assert_eq!(locals, 3);
}
