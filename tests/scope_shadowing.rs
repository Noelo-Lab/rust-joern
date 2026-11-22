use rust_joern::{analyze, graph::Options};

#[test]
fn nested_lambda_shadowing_restores_the_outer_callable() {
    let source = r#"
        int f(int x) {
          auto call = [](int a) { return a + 1; };
          { auto call = [](int a) { return a > 2; }; call(x); }
          return call(x);
        }
    "#;
    let analysis = analyze(
        source,
        "scope.cpp",
        &Options {
            language: Some("cpp".into()),
            preprocessed: true,
            strict: true,
            ..Options::default()
        },
    )
    .unwrap();
    let function = analysis.functions.iter().find(|f| f.name == "f").unwrap();
    let calls: Vec<_> = function
        .cpg
        .nodes
        .iter()
        .filter(|n| n.kind == "CALL" && n.name.as_deref() == Some("<operator>()"))
        .map(|n| n.type_name.as_deref())
        .collect();
    assert_eq!(calls, [Some("bool"), Some("int")]);
}

#[test]
fn for_initializer_names_are_not_function_references() {
    // c2cpg pops the initializer scope before the condition, but CDT still
    // resolves `last` to the loop local rather than the function.
    let source = r#"
        int last(void);
        int (*f(int n))(void) {
          for (int it = 0, last = n; it != last; ++it) { n += last; }
          return last;
        }
    "#;
    for language in ["c", "cpp"] {
        let analysis = analyze(
            source,
            "scope.c",
            &Options {
                language: Some(language.into()),
                preprocessed: true,
                strict: true,
                ..Options::default()
            },
        )
        .unwrap();
        let function = analysis.functions.iter().find(|f| f.name == "f").unwrap();
        let kinds: Vec<_> = function
            .cpg
            .nodes
            .iter()
            .filter(|n| n.code == "last")
            .map(|n| n.kind.as_str())
            .collect();
        assert_eq!(
            kinds,
            ["IDENTIFIER", "IDENTIFIER", "IDENTIFIER", "METHOD_REF"],
            "{language}"
        );
    }
}
