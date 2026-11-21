//! `T{...}` in an expression is a braced temporary even when `T` is unknown.
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
fn undeclared_types_in_expressions_build_temporaries() {
    for source in [
        "long g(long dt) {\n  return f(a::b::c{ dt }).count();\n}\n",
        "long g(long dt) {\n  return f(b{ dt });\n}\n",
        "long g(long dt) {\n  x = a::b::c{ dt };\n  return x;\n}\n",
    ] {
        let analysis = analyze(source, "t.cpp", &options())
            .unwrap_or_else(|error| panic!("{source}: {error}"));
        assert!(
            analysis.functions[0]
                .cpg
                .nodes
                .iter()
                .any(|node| node.name.as_deref() == Some("<operator>.arrayInitializer")),
            "{source}"
        );
    }
}

#[test]
fn blocks_after_conditions_and_lambdas_are_unchanged() {
    let analysis = analyze(
        "void g(int x) {\n  if (x) { x = 1; }\n  while (x) { x--; }\n  auto l = [x](int y) { return x + y; };\n  l(1);\n}\n",
        "t.cpp",
        &options(),
    )
    .unwrap();
    assert!(
        analysis.functions[0]
            .cpg
            .nodes
            .iter()
            .all(|node| node.name.as_deref() != Some("<operator>.arrayInitializer"))
    );
}
