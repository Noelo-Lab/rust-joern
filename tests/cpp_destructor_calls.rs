//! Explicit destructor calls retain receiver identity and evaluation count.
use rust_joern::{
    analyze,
    graph::{Analysis, FunctionGraph, Options},
};

fn parse(source: &str) -> Analysis {
    analyze(
        source,
        "destructor.cpp",
        &Options {
            language: Some("cpp".into()),
            preprocessed: true,
            strict: true,
            ..Options::default()
        },
    )
    .unwrap()
}
fn method<'a>(analysis: &'a Analysis, name: &str) -> &'a FunctionGraph {
    analysis.functions.iter().find(|f| f.name == name).unwrap()
}
fn call(function: &FunctionGraph, name: &str) -> u32 {
    let nodes: Vec<_> = function
        .cpg
        .nodes
        .iter()
        .filter(|n| n.kind == "CALL" && n.name.as_deref() == Some(name))
        .collect();
    assert_eq!(nodes.len(), 1, "one {name}");
    nodes[0].id
}
fn reaches(function: &FunctionGraph, a: u32, b: u32) -> bool {
    let mut pending = vec![a];
    let mut visited = std::collections::HashSet::new();
    while let Some(node) = pending.pop() {
        if node == b {
            return true;
        }
        if visited.insert(node) {
            pending.extend(
                function
                    .cpg
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
fn pointer_and_reference_destructors_keep_the_receiver_operand() {
    let analysis = parse(
        "struct Object { ~Object(); }; void after(); void pointer(Object* p) { p->~Object(); after(); } void reference(Object& r) { r.~Object(); after(); }",
    );
    for (name, receiver, expression) in [
        ("pointer", "p", "p->~Object()"),
        ("reference", "r", "r.~Object()"),
    ] {
        let f = method(&analysis, name);
        let dtor = call(f, "~Object");
        assert_eq!(f.cpg.nodes[dtor as usize].code, expression);
        let edge = f
            .cpg
            .edges
            .iter()
            .find(|e| e.kind == "ARGUMENT" && e.source == dtor && e.label.as_deref() == Some("0"))
            .unwrap();
        assert_eq!(f.cpg.nodes[edge.target as usize].code, receiver);
        assert!(
            !f.cpg
                .nodes
                .iter()
                .any(|n| n.kind == "CALL" && n.name.as_deref() == Some("Object"))
        );
        assert!(reaches(f, dtor, call(f, "after")));
    }
}

#[test]
fn side_effecting_receiver_is_evaluated_once_before_destruction() {
    let analysis = parse(
        "struct Object { ~Object(); }; Object* next(); void after(); void f() { next()->~Object(); after(); }",
    );
    let f = method(&analysis, "f");
    let next = call(f, "next");
    let dtor = call(f, "~Object");
    assert!(reaches(f, next, dtor));
    assert!(reaches(f, dtor, call(f, "after")));
    let edge = f
        .cpg
        .edges
        .iter()
        .find(|e| e.kind == "ARGUMENT" && e.source == dtor && e.label.as_deref() == Some("0"))
        .unwrap();
    assert_eq!(edge.target, next);
}

#[test]
fn qualified_template_destructor_retains_complete_name_and_receiver() {
    let analysis = parse(
        "template<class T> struct Box { ~Box(); }; Box<int>* next(); void f() { next()->Box<int>::~Box(); }",
    );
    let f = method(&analysis, "f");
    let dtor = call(f, "~Box");
    assert_eq!(f.cpg.nodes[dtor as usize].code, "next()->Box<int>::~Box()");
    assert!(reaches(f, call(f, "next"), dtor));
}

#[test]
fn bitwise_complement_and_operator_complement_are_not_destructors() {
    let analysis = parse(
        "struct Object { int operator~() const; }; int complement(int x) { return ~x; } int operator_call(Object& r) { return r.operator~(); }",
    );
    call(method(&analysis, "complement"), "<operator>.not");
    call(method(&analysis, "operator_call"), "operator~");
}

#[test]
fn destructor_arguments_and_missing_call_are_explicitly_rejected() {
    for code in [
        "p->~Object(tick());",
        "p->~Object;",
        "p->Object::~Object(tick());",
    ] {
        let source =
            format!("struct Object {{ ~Object(); }}; int tick(); void f(Object* p) {{ {code} }}");
        let result = analyze(
            &source,
            "invalid.cpp",
            &Options {
                language: Some("cpp".into()),
                strict: true,
                preprocessed: true,
                ..Options::default()
            },
        );
        assert!(result.is_err(), "{code}");
    }
}

#[test]
fn malformed_missing_name_does_not_invent_a_destructor() {
    let analysis =
        parse("struct Object { ~Object(); }; void after(); void f(Object* p) { p->~(); after(); }");
    let f = method(&analysis, "f");
    call(f, "after");
    assert!(
        !f.cpg
            .nodes
            .iter()
            .any(|n| n.kind == "CALL" && n.name.as_deref() == Some("~Object"))
    );
}

#[test]
fn short_circuit_can_bypass_receiver_and_destruction() {
    let analysis = parse(
        "struct Object { ~Object(); }; bool guard(); Object* next(); void after(); void f() { guard() && (next()->~Object(), true); after(); }",
    );
    let f = method(&analysis, "f");
    let guard = call(f, "guard");
    let next = call(f, "next");
    let dtor = call(f, "~Object");
    let after = call(f, "after");
    assert!(reaches(f, guard, next));
    assert!(reaches(f, next, dtor));
    assert!(reaches(f, dtor, after));
    let mut pending = vec![guard];
    let mut visited = std::collections::HashSet::new();
    let mut bypass = false;
    while let Some(node) = pending.pop() {
        if node == next || node == dtor {
            continue;
        }
        if node == after {
            bypass = true;
            break;
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
    assert!(bypass);
}

#[test]
fn indexed_receiver_preserves_each_side_effect_once() {
    let analysis = parse(
        "struct Object { ~Object(); }; Object* objects(); int index(); void after(); void f() { objects()[index()].~Object(); after(); }",
    );
    let f = method(&analysis, "f");
    let dtor = call(f, "~Object");
    assert!(reaches(f, call(f, "objects"), dtor));
    assert!(reaches(f, call(f, "index"), dtor));
    assert!(reaches(f, dtor, call(f, "after")));
}

#[test]
fn destructor_spelling_preserves_source_but_canonicalizes_the_member_name() {
    for spelling in [
        "p->~ Object()",
        "p->compl Object()",
        "p->Object::compl Object()",
    ] {
        let source = format!("struct Object {{ ~Object(); }}; void f(Object* p) {{ {spelling}; }}");
        let analysis = parse(&source);
        let f = method(&analysis, "f");
        let id = call(f, "~Object");
        assert_eq!(f.cpg.nodes[id as usize].code, spelling);
        assert!(
            !f.cpg
                .nodes
                .iter()
                .any(|n| n.kind == "CALL" && n.name.as_deref() == Some("Object"))
        );
        let receiver = f
            .cpg
            .edges
            .iter()
            .find(|e| e.kind == "ARGUMENT" && e.source == id && e.label.as_deref() == Some("0"))
            .unwrap();
        assert_eq!(f.cpg.nodes[receiver.target as usize].code, "p");
    }
}
