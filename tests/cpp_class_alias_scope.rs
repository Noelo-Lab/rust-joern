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
fn inner_alias_does_not_replace_enclosing_alias() {
    for decl in ["typedef int Value;", "using Value = int;"] {
        let source = format!(
            "typedef long Value; struct S {{ {decl} Value local(Value x) {{ return x + 1; }} }}; Value after(Value x) {{ return x; }}"
        );
        let analysis = analyze(&source, "scope.cpp", &options()).unwrap();
        let local = analysis
            .functions
            .iter()
            .find(|f| f.name == "local")
            .unwrap();
        let after = analysis
            .functions
            .iter()
            .find(|f| f.name == "after")
            .unwrap();
        assert_eq!(local.fullname, "S.local:int(int)");
        assert_eq!(after.fullname, "after:long int(long int)");
    }
}

#[test]
fn nested_alias_and_qualified_class_type_keep_their_scope() {
    let source = "typedef long Value; struct S { typedef int Value; struct Inner { typedef char Value; }; Value inside(Value x) { return x; } }; Value outside(Value x) { return x; } S::Value qualified(S::Value x) { return x; }";
    let analysis = analyze(source, "scope.cpp", &options()).unwrap();
    let names: Vec<_> = analysis
        .functions
        .iter()
        .map(|f| f.fullname.as_str())
        .collect();
    assert!(names.contains(&"S.inside:int(int)"));
    assert!(names.contains(&"outside:long int(long int)"));
    assert!(names.contains(&"qualified:S.Value(S.Value)"));
}

#[test]
fn class_only_alias_does_not_leak_to_enclosing_scope() {
    for declaration in ["using Local = int;", "typedef int Local;"] {
        // Unresolved names can occur in incomplete input. They must not bind
        // to an alias that was only visible inside the class.
        let source = format!("struct S {{ {declaration} }}; Local f(Local x) {{ return x; }}");
        let result = analyze(&source, "scope.cpp", &options()).unwrap();
        let f = result.functions.iter().find(|f| f.name == "f").unwrap();
        assert_eq!(f.fullname, "f:ANY(ANY)", "{declaration}");
    }
}
