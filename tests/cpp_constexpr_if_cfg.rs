use rust_joern::{
    analyze,
    graph::{FunctionGraph, Options},
};

fn options() -> Options {
    Options {
        language: Some("cpp".into()),
        strict: true,
        ..Options::default()
    }
}

fn has_cfg_call(function: &FunctionGraph, code: &str) -> bool {
    function.cpg.nodes.iter().any(|node| {
        node.kind == "CALL"
            && node.code == code
            && function
                .cfg
                .nodes
                .iter()
                .any(|block| block.statements.contains(&node.id))
    })
}

#[test]
fn literal_constexpr_conditions_keep_only_the_selected_control_flow() {
    let source = "int one(int); int two(int); int after(int);
        int first(int x) {
            if constexpr (true) return one(x);
            else { two(x); }
        }
        int second(int x) {
            if constexpr ((0)) return one(x);
            else { two(x); }
            return after(x);
        }
        int no_else(int x) {
            if constexpr (false) return one(x);
            return after(x);
        }";
    let result = analyze(source, "constexpr.cpp", &options()).unwrap();
    let get = |name| result.functions.iter().find(|f| f.name == name).unwrap();
    for (name, present, absent) in [
        ("first", "one(x)", "two(x)"),
        ("second", "two(x)", "one(x)"),
        ("no_else", "after(x)", "one(x)"),
    ] {
        let function = get(name);
        assert!(has_cfg_call(function, present), "{name}: missing {present}");
        assert!(!has_cfg_call(function, absent), "{name}: executes {absent}");
        assert!(function.cpg.nodes.iter().any(|n| n.code == absent));
    }
    assert!(has_cfg_call(get("second"), "after(x)"));
}

#[test]
fn runtime_if_and_joern_constexpr_mode_keep_both_edges() {
    let source = "int one(int); int two(int);
        int runtime(int x) { if (true) return one(x); else return two(x); }
        int compiletime(int x) { if constexpr (true) return one(x); else return two(x); }";
    for (compat, expected_compiletime_else) in [(false, false), (true, true)] {
        let result = analyze(
            source,
            "constexpr.cpp",
            &Options {
                joern_cpp_control_flow: compat,
                ..options()
            },
        )
        .unwrap();
        let get = |name| result.functions.iter().find(|f| f.name == name).unwrap();
        assert!(has_cfg_call(get("runtime"), "one(x)"));
        assert!(has_cfg_call(get("runtime"), "two(x)"));
        assert!(has_cfg_call(get("compiletime"), "one(x)"));
        assert_eq!(
            has_cfg_call(get("compiletime"), "two(x)"),
            expected_compiletime_else
        );
    }
}

#[test]
fn a_dependent_condition_is_a_diagnostic_except_in_compatibility_mode() {
    let source = "int one(int); int two(int);
        template<bool Pick> int choose(int x) {
            if constexpr (Pick) return one(x); else return two(x);
        }";
    let message = analyze(source, "constexpr.cpp", &options()).unwrap_err();
    assert!(message.contains("if constexpr condition requires compile-time evaluation"));
    let compat = analyze(
        source,
        "constexpr.cpp",
        &Options {
            joern_cpp_control_flow: true,
            ..options()
        },
    )
    .unwrap();
    let function = compat
        .functions
        .iter()
        .find(|f| f.name == "choose")
        .unwrap();
    assert!(has_cfg_call(function, "one(x)"));
    assert!(has_cfg_call(function, "two(x)"));
}

#[test]
fn a_runtime_init_statement_never_disappears_as_a_literal_condition() {
    let source = "int init(); int one(int); int two(int);
        int with_init() { if constexpr (int x = init(); true) return one(x); else return two(x); }";
    let result = analyze(
        source,
        "constexpr.cpp",
        &Options {
            strict: false,
            ..options()
        },
    )
    .unwrap();
    assert!(result.diagnostics.iter().any(|d| {
        d.message
            .contains("if constexpr condition requires compile-time evaluation")
    }));
    let function = result
        .functions
        .iter()
        .find(|f| f.name == "with_init")
        .unwrap();
    assert!(has_cfg_call(function, "init()"));
}

#[test]
fn discarded_subtrees_do_not_require_dependent_cfgs() {
    let source = "int one(int); int two(int);
        template<bool Pick> int nested(int x) {
            if constexpr (true) return one(x);
            else if constexpr (Pick) return two(x);
        }";
    let result = analyze(source, "constexpr.cpp", &options()).unwrap();
    let function = result
        .functions
        .iter()
        .find(|f| f.name == "nested")
        .unwrap();
    assert!(has_cfg_call(function, "one(x)"));
    assert!(!has_cfg_call(function, "two(x)"));
}
