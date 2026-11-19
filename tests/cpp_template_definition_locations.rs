use rust_joern::{analyze, graph::Options};

#[test]
fn template_definitions_start_after_the_template_head() {
    let source = r#"int called(int value) { return value; }

template <class T>
static int
multiline(int value) {
    return called(value);
}

template <class T> int same_line(int value) { return value; }

template <class T>
int prototype(T value);

int
non_template(int value) { return called(value); }

template <class T>
struct Record {
    template <class U>
    int member(int value, int other) { return called(value + other); }
};

template <class T> int specialized(T value) { return value; }
template <>
int specialized(int value) { return called(value); }
"#;
    let analysis = analyze(
        source,
        "locations.cpp",
        &Options {
            preprocessed: true,
            strict: true,
            ..Options::default()
        },
    )
    .unwrap();

    // Original Joern 4.0.150 reports the inner function definition's
    // location, including the specifiers and any indentation.
    for (name, line, column, end, first_token) in [
        ("multiline:int(int)", 4, 1, 7, "static int\n"),
        ("same_line:int(int)", 9, 20, 9, "int same_line"),
        ("non_template:int(int)", 14, 1, 15, "int\n"),
        ("Record.member:int(int,int)", 20, 5, 20, "int member"),
        ("specialized:int(int)", 25, 1, 25, "int specialized"),
    ] {
        let function = analysis
            .functions
            .iter()
            .find(|function| function.fullname == name)
            .unwrap_or_else(|| panic!("missing {name}"));
        let method = function
            .cpg
            .nodes
            .iter()
            .find(|node| node.kind == "METHOD")
            .unwrap();
        assert_eq!(
            (function.start_line, function.end_line),
            (line, end),
            "{name}"
        );
        assert_eq!((method.line, method.column), (line, column), "{name}");
        let method_source = &source[method.start_byte..method.end_byte];
        assert!(method_source.starts_with(first_token), "{method_source}");
        assert!(method_source.ends_with('}'), "{method_source}");
        assert!(method.code.starts_with(first_token), "{name}");
        assert!(function.cpg.nodes.iter().any(|node| node.kind == "RETURN"));
    }
}
