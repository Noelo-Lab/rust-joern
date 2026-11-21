//! A pack expansion `x...` ends its element; GNU case ranges still parse.
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
fn pack_expansions_parse_without_diagnostics() {
    for source in [
        "template <int... seq>\nint sum(int n) {\n  int values[] = {f(seq)...};\n  return values[0] + n;\n}\n",
        "template <int... seq>\nA<seq...> make() {\n  return A<seq...>{f(seq)...};\n}\n",
        "template <class... T>\nint g(T... v) {\n  return h(v...);\n}\n",
    ] {
        analyze(source, "t.cpp", &options()).unwrap_or_else(|error| panic!("{source}: {error}"));
    }
}

#[test]
fn gnu_case_ranges_keep_both_bounds() {
    let analysis = analyze(
        "int g(int a, int b) {\n  switch (a) { case 1 ... 3: return b; }\n  return 0;\n}\n",
        "t.cpp",
        &options(),
    )
    .unwrap();
    assert!(
        analysis.functions[0]
            .cpg
            .nodes
            .iter()
            .any(|n| n.code == "3")
    );
}
