//! Recover explicit bodies in incomplete slices without inventing class facts.
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
fn qualified_destructor_retains_body_without_class_layout() {
    let source = "demo::Record::~Record() { release(); }";
    let analysis = analyze(source, "slice.cpp", &options()).unwrap();
    assert_eq!(analysis.functions.len(), 1);
    let method = &analysis.functions[0];
    assert_eq!(method.name, "~Record");
    assert_eq!(
        method
            .cpg
            .nodes
            .iter()
            .filter(|n| n.name.as_deref() == Some("release"))
            .count(),
        1
    );
    assert!(
        !method
            .cpg
            .nodes
            .iter()
            .any(|n| n.name.as_deref() == Some("this"))
    );
}

#[test]
fn qualified_constructor_retains_explicit_calls_without_a_field_guess() {
    let source = "demo::Record::Record() { first(); second(); }";
    let analysis = analyze(source, "slice.cpp", &options()).unwrap();
    assert_eq!(analysis.functions.len(), 1);
    let method = &analysis.functions[0];
    assert_eq!(method.name, "Record");
    for name in ["first", "second"] {
        assert_eq!(
            method
                .cpg
                .nodes
                .iter()
                .filter(|n| n.name.as_deref() == Some(name))
                .count(),
            1
        );
    }
    assert!(
        !method
            .cpg
            .nodes
            .iter()
            .any(|n| n.name.as_deref() == Some("this"))
    );
}

#[test]
fn ordinary_qualified_omitted_return_is_not_a_constructor() {
    let analysis = analyze("demo::Record::run() { effect(); }", "slice.cpp", &options()).unwrap();
    assert!(analysis.functions.is_empty());
    let mismatch = analyze(
        "demo::Record::~Other() { effect(); }",
        "slice.cpp",
        &options(),
    )
    .unwrap();
    assert!(mismatch.functions.is_empty());
}

#[test]
fn recovery_does_not_qualify_unknown_initialization_effects() {
    assert!(
        analyze(
            "demo::Record::Record(int value) : field(value) {}",
            "slice.cpp",
            &options()
        )
        .is_err()
    );
}
