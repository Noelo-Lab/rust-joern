use rust_joern::{analyze, graph::Options};

#[test]
fn primary_template_member_definition_retains_its_body_and_owner() {
    let source = "namespace demo {
        int tick(int);
        template<class T, typename U> struct Buffer {
            int append(int n);
            int tail(int n) { return n; }
        };
        template<typename A, class B>
        int Buffer<A, B>::append(int n) {
            while (n > 0) n = tick(n);
            return n;
        }
        template<>
        int Buffer<double, int>::append(int n) { return n + 7; }
    }";
    let analysis = analyze(
        source,
        "template-member.cpp",
        &Options {
            language: Some("cpp".into()),
            strict: true,
            ..Options::default()
        },
    )
    .unwrap();
    let definitions: Vec<_> = analysis
        .functions
        .iter()
        .filter(|f| f.name == "append" && f.start_line < f.end_line)
        .collect();
    let [method] = definitions.as_slice() else {
        panic!("expected one multiline primary definition, got {definitions:?}");
    };
    assert_eq!(method.fullname, "demo.Buffer.append:int(int)");
    assert_eq!((method.start_line, method.end_line), (7, 11));
    assert!(method.cfg.edges.len() >= 3);
    assert!(
        method
            .cpg
            .nodes
            .iter()
            .any(|n| { n.kind == "CALL" && n.name.as_deref() == Some("tick") })
    );
    assert!(
        method
            .cpg
            .nodes
            .iter()
            .any(|n| n.kind == "RETURN" && n.code == "return n;")
    );
    assert!(
        analysis
            .functions
            .iter()
            .any(|f| f.name == "tail" && f.cpg.nodes.iter().any(|n| n.kind == "RETURN"))
    );
}
