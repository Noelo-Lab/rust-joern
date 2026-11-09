use rust_joern::{analyze, graph::Options};

#[test]
fn scoped_enums_do_not_consume_the_following_function() {
    for (key, base) in [("class", " : unsigned char"), ("struct", ""), ("class", "")] {
        let source = format!(
            "enum {key} E{base} {{ none=0, debug=1, other }}; int f(E e) {{ if (e == E::none) return 1; return 0; }}"
        );
        let options = Options {
            language: Some("cpp".into()),
            preprocessed: true,
            strict: true,
            ..Options::default()
        };
        let analysis = analyze(&source, "enum.cpp", &options).expect("valid scoped enum");
        let function = analysis
            .functions
            .iter()
            .find(|f| f.name == "f")
            .expect("following function");
        let entry = function.cfg.nodes.iter().find(|n| n.is_entrypoint).unwrap();
        assert_eq!(function.cfg.nodes.len(), 3);
        assert_eq!(function.cfg.edges.len(), 2);
        assert_eq!(
            function
                .cfg
                .edges
                .iter()
                .filter(|edge| edge[0] == entry.id)
                .count(),
            2
        );
    }
}

#[test]
fn scoped_enum_does_not_hide_actual_record_methods() {
    let source = "enum class E { one }; struct S { int f() { return 1; } }; int g() { return 2; }";
    let options = Options {
        language: Some("cpp".into()),
        preprocessed: true,
        strict: true,
        ..Options::default()
    };
    let analysis = analyze(source, "enum.cpp", &options).expect("valid declarations");
    assert!(analysis.functions.iter().any(|f| f.name == "f"));
    assert!(analysis.functions.iter().any(|f| f.name == "g"));
}
