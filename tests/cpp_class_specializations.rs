use rust_joern::{analyze, graph::Options};

#[test]
fn class_specialization_methods_keep_their_own_fields_and_calls() {
    let source = "namespace demo {
struct Tag {};
template<class T> struct Box {
    int primary;
    int get() const { return primary; }
    int read() const { return get(); }
};
template<> struct Box<int> {
    long counter;
    long get() const { return counter; }
    int read() const { if (counter) return get(); return 0; }
};
template<> struct Box< ::demo::Tag> {
    char marker;
    char get() const { return marker; }
    int read() const { return get(); }
};
int after(Box<double>& box) { return box.read(); }
}";
    let result = analyze(
        source,
        "variants.cpp",
        &Options {
            strict: true,
            ..Options::default()
        },
    )
    .unwrap();
    for (get_line, read_line, ty, field) in [
        (5, 6, "int", "primary"),
        (10, 11, "long int", "counter"),
        (15, 16, "char", "marker"),
    ] {
        let get = result
            .functions
            .iter()
            .find(|f| f.name == "get" && f.start_line == get_line)
            .unwrap();
        assert_eq!(get.fullname, format!("demo.Box.get:{ty}()"));
        assert!(
            get.cpg
                .nodes
                .iter()
                .any(|n| n.kind == "FIELD_IDENTIFIER" && n.code == field)
        );

        let read = result
            .functions
            .iter()
            .find(|f| f.name == "read" && f.start_line == read_line)
            .unwrap();
        assert_eq!(read.fullname, "demo.Box.read:int()");
        assert!(
            read.cpg.nodes.iter().any(|n| n.kind == "CALL"
                && n.method_full_name.as_deref() == Some(get.fullname.as_str()))
        );
    }
    assert!(
        result
            .functions
            .iter()
            .any(|f| f.name == "after" && f.cpg.nodes.iter().any(|n| n.kind == "RETURN"))
    );
}

#[test]
fn partial_specializations_and_grouped_arguments_preserve_bodies() {
    let source = "int tick(int);
template<class T> struct Print {};
template<class T, unsigned N> struct Print<T[N]> {
    static int format(int n) { return tick(n); }
};
template<class T> struct Print<T*> {
    static int pointer(int n) { return tick(n + 1); }
};
template<int N> struct Count {};
template<> struct [[nodiscard]] Count<(2 > 1 ? 2 : 1)> final : Print<int[2]> {
    static int count(int n) { return tick(n + 2); }
};
int tail(int n) { return tick(n + 3); }";
    let result = analyze(
        source,
        "partial.cpp",
        &Options {
            strict: true,
            ..Options::default()
        },
    )
    .unwrap();
    for key in [
        "Print.format:int(int)",
        "Print.pointer:int(int)",
        "Count.count:int(int)",
        "tail:int(int)",
    ] {
        let function = result.functions.iter().find(|f| f.fullname == key).unwrap();
        assert!(
            function.cpg.nodes.iter().any(|node| node.kind == "RETURN"),
            "{key}"
        );
        assert!(
            function
                .cpg
                .nodes
                .iter()
                .any(|node| node.name.as_deref() == Some("tick")),
            "{key}"
        );
    }
}
