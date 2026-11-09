use rust_joern::{analyze, graph::Options};

fn method_refs(source: &str, name: &str) -> usize {
    let options = Options {
        language: Some("cpp".into()),
        strict: true,
        ..Options::default()
    };
    let analysis = analyze(source, "qualifier.cpp", &options).unwrap();
    let function = analysis.functions.iter().find(|f| f.name == name).unwrap();
    function
        .cpg
        .nodes
        .iter()
        .filter(|node| node.kind == "METHOD_REF")
        .count()
}

// A qualifier spelling the enclosing class's constructor names the class. Joern
// has no METHOD_REF here, and one would add a function-start marker to the CFG.
#[test]
fn scope_qualifier_is_not_a_constructor_reference() {
    let source = "struct Node { enum { mapped = 1 }; Node(int s); int f(int a); };
        Node::Node(int s) { }
        int Node::f(int a) { int flags = Node::mapped; return flags + a; }";
    assert_eq!(method_refs(source, "f"), 0);
}

#[test]
fn isolated_scope_qualifier_is_not_a_constructor_reference() {
    let source = "Node::Node(int s) { }
        int Node::f(int a) { int flags = Node::mapped; return flags + a; }";
    assert_eq!(method_refs(source, "f"), 0);
}

#[test]
fn unqualified_function_name_is_still_a_reference() {
    let source = "void callee(int a);
        void take(void (*p)(int));
        void f(int a) { take(callee); }";
    assert_eq!(method_refs(source, "f"), 1);
}
