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

#[test]
fn class_template_dependent_types_bind_as_any() {
    // CDT types the injected class name and template-ids with a dependent
    // argument as dependent; Joern prints them as `ANY`, so the copy, move
    // and `Span<T>` constructors share one full name.
    let source = "template <class E> struct Span { E *first; };
template <class T> struct Buffer {
    Buffer(const Buffer &o) { n = o.n; }
    Buffer(Buffer &&o) { n = o.n; }
    Buffer(Span<T> s) { n = 0; }
    Buffer(Span<int> s, int x) { n = x; }
    int total(const Buffer<int> &o) const { return o.n; }
    Buffer *self() { return this; }
    int n;
};
struct Plain { Plain(const Plain &o) { n = o.n; } int n; };";
    let analysis = analyze(source, "template.cpp", &options()).unwrap();
    let names: Vec<_> = analysis
        .functions
        .iter()
        .map(|f| f.fullname.as_str())
        .filter(|name| !name.ends_with("<global>"))
        .collect();
    assert_eq!(
        names,
        [
            "Buffer.Buffer:ANY(ANY)",
            "Buffer.Buffer:ANY(ANY)",
            "Buffer.Buffer:ANY(ANY)",
            "Buffer.Buffer:ANY(Span<int>,int)",
            "Buffer.total:int(Buffer<int> &)",
            "Buffer.self:ANY()",
            "Plain.Plain:ANY(Plain &)",
        ]
    );
}
