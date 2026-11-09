use rust_joern::{analyze, graph::Options};

#[test]
fn provided_type_name_is_explicitly_acknowledged() {
    let source = "int consume(); int f(){ return consume(Options{}); }";
    let mut options = Options {
        language: Some("cpp".into()),
        strict: true,
        ..Options::default()
    };
    assert!(analyze(source, "input.cpp", &options).is_err());
    options.cpp_type_names = vec!["Options".into()];
    let result = analyze(source, "input.cpp", &options).unwrap();
    assert_eq!(result.cpp_type_names, vec!["Options"]);
    assert!(result.functions.iter().any(|f| f.name == "f"));
}

#[test]
fn value_binding_does_not_become_a_supplied_type() {
    let options = Options {
        language: Some("cpp".into()),
        strict: true,
        cpp_type_names: vec!["Options".into()],
        ..Options::default()
    };
    assert!(
        analyze(
            "int f(int Options){ return Options{}; }",
            "input.cpp",
            &options
        )
        .is_err()
    );
}

#[test]
fn context_requires_cpp_and_identifier_shaped_names() {
    for name in ["", "A<B>", "A::", "int*", "A; int x"] {
        let options = Options {
            language: Some("cpp".into()),
            cpp_type_names: vec![name.into()],
            ..Options::default()
        };
        assert!(
            analyze("int f(){ return 0; }", "x.cpp", &options).is_err(),
            "{name:?}"
        );
    }
    let options = Options {
        language: Some("c".into()),
        cpp_type_names: vec!["A".into()],
        ..Options::default()
    };
    assert!(analyze("int f(){return 0;}", "x.c", &options).is_err());
}

#[test]
fn empty_context_is_omitted_from_serialized_analysis() {
    let result = analyze("int f(){return 0;}", "x.cpp", &Options::default()).unwrap();
    assert!(
        !serde_json::to_value(result)
            .unwrap()
            .as_object()
            .unwrap()
            .contains_key("cpp_type_names")
    );
}
