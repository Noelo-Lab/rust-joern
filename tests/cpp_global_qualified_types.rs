use rust_joern::{analyze, graph::Options};

#[test]
fn leading_global_qualifier_preserves_function_bodies() {
    let source = "struct Root {};
namespace util { struct Value {}; }
struct Ordinary {
    static ::util::Value absolute(util::Value value) { return value; }
    static util::Value relative(util::Value value) { return value; }
};
::Root global_absolute(Root value) { return value; }";
    let result = analyze(
        source,
        "qualified.cpp",
        &Options {
            strict: true,
            ..Options::default()
        },
    )
    .unwrap();
    for (name, fullname) in [
        ("absolute", "Ordinary.absolute:util.Value(util.Value)"),
        ("relative", "Ordinary.relative:util.Value(util.Value)"),
        ("global_absolute", "global_absolute:Root(Root)"),
    ] {
        let function = result.functions.iter().find(|f| f.name == name).unwrap();
        assert_eq!(function.fullname, fullname);
        assert!(function.cpg.nodes.iter().any(|n| n.kind == "RETURN"));
    }
}
