//! Call-operand contracts, including effects that CFG degree cannot establish.
use rust_joern::{
    analyze,
    graph::{Analysis, FunctionGraph, Options},
};

fn parse(source: &str) -> Analysis {
    analyze(
        source,
        "calls.cpp",
        &Options {
            language: Some("cpp".into()),
            strict: true,
            preprocessed: true,
            ..Options::default()
        },
    )
    .unwrap()
}

fn method<'a>(analysis: &'a Analysis, name: &str) -> &'a FunctionGraph {
    analysis.functions.iter().find(|f| f.name == name).unwrap()
}

fn call(f: &FunctionGraph, name: &str) -> u32 {
    let nodes: Vec<_> = f
        .cpg
        .nodes
        .iter()
        .filter(|n| n.kind == "CALL" && n.name.as_deref() == Some(name))
        .collect();
    assert_eq!(nodes.len(), 1, "expected one {name}");
    nodes[0].id
}

fn edge(f: &FunctionGraph, source: u32, kind: &str, label: Option<&str>) -> Option<u32> {
    let edges: Vec<_> = f
        .cpg
        .edges
        .iter()
        .filter(|e| e.source == source && e.kind == kind && e.label.as_deref() == label)
        .collect();
    assert!(edges.len() <= 1, "ambiguous {kind} {label:?}");
    edges.first().map(|e| e.target)
}

fn reaches(f: &FunctionGraph, from: u32, to: u32, avoid: Option<u32>) -> bool {
    let mut pending = vec![from];
    let mut visited = std::collections::HashSet::new();
    while let Some(node) = pending.pop() {
        if Some(node) == avoid {
            continue;
        }
        if node == to {
            return true;
        }
        if visited.insert(node) {
            pending.extend(
                f.cpg
                    .edges
                    .iter()
                    .filter(|e| e.kind == "CFG" && e.source == node)
                    .map(|e| e.target),
            );
        }
    }
    false
}

#[test]
fn unresolved_member_shares_one_receiver_ast_and_evaluation() {
    let analysis = parse("int tick(); int f() { return factory()->missing(tick()); }");
    let f = method(&analysis, "f");
    let missing = call(f, "missing");
    let factory = call(f, "factory");
    let tick = call(f, "tick");
    assert_eq!(edge(f, missing, "ARGUMENT", Some("0")), Some(factory));
    assert_eq!(edge(f, missing, "RECEIVER", None), Some(factory));
    assert_eq!(edge(f, missing, "ARGUMENT", Some("1")), Some(tick));
    assert_eq!(
        f.cpg
            .edges
            .iter()
            .filter(|e| e.source == missing && e.target == factory && e.kind == "AST")
            .count(),
        1
    );
    assert!(reaches(f, factory, tick, None));
    assert!(reaches(f, tick, missing, None));
}

#[test]
fn resolved_nonvirtual_base_is_not_implicitly_promoted_to_receiver() {
    let analysis = parse("struct Box { int method(int); }; int f(Box* p) { return p->method(1); }");
    let f = method(&analysis, "f");
    let id = call(f, "method");
    assert!(edge(f, id, "ARGUMENT", Some("0")).is_some());
    assert!(edge(f, id, "RECEIVER", None).is_none());
}

#[test]
fn explicit_virtual_evidence_adds_receiver_to_same_base() {
    let analysis =
        parse("struct Box { virtual int method(int); }; int f(Box* p) { return p->method(1); }");
    let f = method(&analysis, "f");
    let id = call(f, "method");
    assert_eq!(
        edge(f, id, "RECEIVER", None),
        edge(f, id, "ARGUMENT", Some("0"))
    );
    assert!(edge(f, id, "RECEIVER", None).is_some());
}

#[test]
fn virtual_word_in_method_name_is_not_virtual_evidence() {
    let analysis = parse(
        "struct Box { int virtual_method(int); }; int f(Box* p) { return p->virtual_method(1); }",
    );
    let f = method(&analysis, "f");
    assert!(edge(f, call(f, "virtual_method"), "RECEIVER", None).is_none());
}

#[test]
fn pointer_and_closure_have_receiver_but_no_argument_zero() {
    for (source, name) in [
        (
            "int f(int (*fn)(int)) { return (fn)(1); }",
            "<operator>.pointerCall",
        ),
        (
            "int f(int x) { auto fn = [x](int y) { return x+y; }; return fn(1); }",
            "<operator>()",
        ),
    ] {
        let analysis = parse(source);
        let f = method(&analysis, "f");
        let id = call(f, name);
        assert!(edge(f, id, "RECEIVER", None).is_some());
        assert!(edge(f, id, "ARGUMENT", Some("0")).is_none());
        assert!(edge(f, id, "ARGUMENT", Some("1")).is_some());
    }
}

#[test]
fn unresolved_expression_uses_shared_base_and_receiver() {
    let analysis = parse("int f(int x) { return (missing)(x); }");
    let f = method(&analysis, "f");
    let id = call(f, "<operator>()");
    assert_eq!(
        edge(f, id, "ARGUMENT", Some("0")),
        edge(f, id, "RECEIVER", None)
    );
    assert_eq!(
        f.cpg.nodes[id as usize].method_full_name.as_deref(),
        Some("<unresolvedNamespace>.<operator>():<unresolvedSignature>(1)")
    );
}

#[test]
fn indexed_receiver_effects_are_unique_and_can_be_bypassed() {
    let analysis = parse(
        "int tick(); int f(int flag, Unknown** p, int i) { return flag && p[i++]->missing(tick()); }",
    );
    let f = method(&analysis, "f");
    let id = call(f, "missing");
    let tick = call(f, "tick");
    let inc = call(f, "<operator>.postIncrement");
    let entry = f.cpg.nodes.iter().find(|n| n.kind == "METHOD").unwrap().id;
    let exit = f
        .cpg
        .nodes
        .iter()
        .find(|n| n.kind == "METHOD_RETURN")
        .unwrap()
        .id;
    assert!(reaches(f, inc, tick, None));
    assert!(reaches(f, tick, id, None));
    assert!(reaches(f, entry, exit, Some(inc)));
    assert!(reaches(f, entry, exit, Some(tick)));
    assert!(reaches(f, entry, exit, Some(id)));
}

#[test]
fn unresolved_overload_keeps_identity_and_argument_positions_without_guessing() {
    let analysis =
        parse("int choose(int); int choose(char*); int f(Unknown value) { return choose(value); }");
    let f = method(&analysis, "f");
    let id = call(f, "choose");
    assert_eq!(
        f.cpg.nodes[id as usize].method_full_name.as_deref(),
        Some("<unresolvedNamespace>.choose:<unresolvedSignature>(1)")
    );
    assert!(edge(f, id, "ARGUMENT", Some("1")).is_some());
    assert!(edge(f, id, "RECEIVER", None).is_none());
}

#[test]
fn generated_allocation_call_keeps_canonical_intrinsic_identity() {
    for source in [
        "int* f() { return new int; }",
        "int* f(int n) { return new int[n]; }",
        "struct Object { Object(int); }; Object* f() { return new Object(1); }",
    ] {
        let analysis = parse(source);
        let f = method(&analysis, "f");
        let id = call(f, "<operator>.new");
        assert_eq!(
            f.cpg.nodes[id as usize].method_full_name.as_deref(),
            Some("<operator>.new")
        );
        assert!(edge(f, id, "ARGUMENT", Some("0")).is_none());
        assert!(edge(f, id, "RECEIVER", None).is_none());
    }
}

#[test]
fn unsupported_known_owner_lookup_is_not_a_proven_problem_binding() {
    for source in [
        "struct Base { int get(int); }; struct Derived : Base {}; int f(Derived* p) { return p->get(1); }",
        "struct Box { int get(int); int get(char*); }; int f(Box* p, Unknown value) { return p->get(value); }",
        "struct Box { int get(int); }; Box* factory(); int f() { return factory()->get(1); }",
        "struct Base { int get(int); }; struct Derived : Base {}; int f(Derived* p) { return p->Base::get(1); }",
        "struct Box { int get(int); }; int f(Box** p, int i) { return p[i]->get(1); }",
    ] {
        let analysis = parse(source);
        let f = method(&analysis, "f");
        let id = call(f, "get");
        assert!(edge(f, id, "ARGUMENT", Some("0")).is_some());
        assert!(edge(f, id, "RECEIVER", None).is_none());
        assert!(f.cpg.nodes[id as usize].method_full_name.is_none());
    }
}
