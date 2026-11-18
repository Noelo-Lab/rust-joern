use rust_joern::{analyze, graph::Options};

fn options() -> Options {
    Options {
        language: Some("cpp".into()),
        strict: true,
        preprocessed: true,
        ..Options::default()
    }
}

#[test]
fn qualified_nested_class_keeps_body_and_full_owner() {
    let source = include_str!("fixtures/cpp-qualified-records.cpp");
    let result = analyze(source, "qualified.cpp", &options()).unwrap();
    let read = result
        .functions
        .iter()
        .find(|f| f.fullname == "trial.Owner.Nested.read:int()")
        .unwrap();
    assert_eq!((read.start_line, read.end_line), (10, 13));
    assert_eq!(read.cfg.nodes.len(), 3);
    assert_eq!(read.cfg.edges.len(), 2);
    assert!(read.cpg.nodes.iter().any(|n| {
        n.kind == "METHOD_PARAMETER_IN"
            && n.name.as_deref() == Some("this")
            && n.type_name.as_deref() == Some("trial.Owner.Nested")
    }));
    assert!(
        read.cpg
            .nodes
            .iter()
            .any(|n| { n.kind == "FIELD_IDENTIFIER" && n.name.as_deref() == Some("state") })
    );
    assert!(result.functions.iter().any(|f| f.name == "after"));
}

#[test]
fn sibling_namespaces_and_outside_member_definitions_keep_distinct_owners() {
    let source = "
        namespace a { struct Outer { struct Inner; }; }
        namespace b { struct Outer { struct Inner; }; }
        struct a::Outer::Inner { int left() const { return 1; } };
        namespace b {
            struct Outer::Inner { int right() const; };
            int Outer::Inner::right() const { return 2; }
        }";
    let result = analyze(source, "qualified.cpp", &options()).unwrap();
    let names: Vec<_> = result
        .functions
        .iter()
        .map(|f| f.fullname.as_str())
        .collect();
    assert!(names.contains(&"a.Outer.Inner.left:int()"));
    assert!(names.contains(&"b.Outer.Inner.right:int()"));
    assert_eq!(names.len(), 2);
}

#[test]
fn qualified_record_resolves_its_enclosing_class_before_adding_scope() {
    let source = "
        namespace a {
            struct Outer { struct Inner; };
            struct a::Outer::Inner final {
                int value;
                int get() const { return value; }
            };
            int after() { return 3; }
        }";
    let result = analyze(source, "qualified.cpp", &options()).unwrap();
    assert!(
        result
            .functions
            .iter()
            .any(|f| f.fullname == "a.Outer.Inner.get:int()")
    );
    assert!(
        result
            .functions
            .iter()
            .any(|f| f.fullname == "a.after:int()")
    );
    assert!(
        result
            .functions
            .iter()
            .all(|f| !f.fullname.starts_with("a.a."))
    );
}

#[test]
fn qualified_record_aliases_do_not_change_following_functions() {
    let source = "
        struct Owner { struct Nested; };
        struct Owner::Nested {
            typedef char Value;
            int inside(Value x) { return x; }
        };
        using Value = int;
        Value after(Value x) { return x; }";
    let result = analyze(source, "qualified.cpp", &options()).unwrap();
    assert!(
        result
            .functions
            .iter()
            .any(|f| f.fullname == "Owner.Nested.inside:int(char)")
    );
    assert!(
        result
            .functions
            .iter()
            .any(|f| f.fullname == "after:int(int)")
    );
}
