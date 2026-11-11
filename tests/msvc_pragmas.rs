use rust_joern::{analyze, graph::Options};

#[test]
fn inline_pragmas_preserve_function_and_branch_cfg() {
    let source = r#"__pragma(pack(push, 8)) struct Record { int value; };
__pragma(pack(pop))
__pragma(warning(push)) int f(int x) {
    __pragma /* comment */ (warning(
        disable : 4189; once : 4507))
    if (x) return 3;
    __pragma(warning(pop)) return 5;
}
"#;
    let control = "struct Record { int value; }; int f(int x) { if (x) return 3; return 5; }";
    for language in ["c", "cpp"] {
        for preprocessed in [false, true] {
            let options = Options {
                language: Some(language.into()),
                preprocessed,
                strict: true,
                ..Options::default()
            };
            let analysis = analyze(source, "pragma.cpp", &options).unwrap();
            let expected = analyze(control, "pragma.cpp", &options).unwrap();
            assert!(analysis.diagnostics.is_empty());
            let [function] = analysis.functions.as_slice() else {
                panic!("expected one function, got {:?}", analysis.functions);
            };
            let control = &expected.functions[0];
            assert_eq!(function.fullname, control.fullname);
            assert_eq!(function.start_line, 3);
            assert_eq!(function.end_line, 8);
            assert_eq!(function.cfg.edges, control.cfg.edges);
            let statements = |function: &rust_joern::graph::FunctionGraph| {
                function
                    .cfg
                    .nodes
                    .iter()
                    .map(|block| {
                        block
                            .statements
                            .iter()
                            .map(|&id| &function.cpg.nodes[id as usize])
                            .filter(|node| node.kind != "METHOD")
                            .map(|node| node.code.clone())
                            .collect::<Vec<_>>()
                    })
                    .collect::<Vec<_>>()
            };
            assert_eq!(statements(function), statements(control));
        }
    }
}

#[test]
fn unsupported_or_unbalanced_pragmas_remain_diagnostics() {
    for directive in ["__pragma(omp parallel)", "__pragma(warning(push)"] {
        let source = format!("{directive}\nint f() {{ return 3; }}");
        let options = Options {
            language: Some("cpp".into()),
            preprocessed: true,
            ..Options::default()
        };
        let analysis = analyze(&source, "pragma.cpp", &options).unwrap();
        assert!(analysis.diagnostics.iter().any(|diagnostic| {
            diagnostic.line == 1
                && diagnostic.column == 1
                && diagnostic.message == "unsupported or malformed MSVC __pragma"
        }));
        assert!(
            analyze(
                &source,
                "pragma.cpp",
                &Options {
                    strict: true,
                    ..options
                }
            )
            .is_err()
        );
    }
}
