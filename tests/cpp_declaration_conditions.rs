use rust_joern::{analyze, graph::Options};

const SOURCE: &str = "int next(); void use(int);
    int f() {
        switch (int kind = next()) { case 0: return 1; default: use(kind); }
        while (int item = next()) use(item);
        if (int value = next()) use(value);
        return 0;
    }";

fn cfg_codes(joern_cpp_control_flow: bool) -> Vec<String> {
    let options = Options {
        language: Some("cpp".into()),
        strict: true,
        joern_cpp_control_flow,
        ..Options::default()
    };
    let result = analyze(SOURCE, "conditions.cpp", &options).unwrap();
    let graph = &result.functions.iter().find(|f| f.name == "f").unwrap().cpg;
    graph
        .nodes
        .iter()
        .filter(|n| {
            graph
                .edges
                .iter()
                .any(|e| e.kind == "CFG" && (e.source == n.id || e.target == n.id))
        })
        .map(|n| n.code.clone())
        .collect()
}

#[test]
fn declaration_conditions_keep_their_loop_and_switch() {
    let codes = cfg_codes(false);
    for code in ["kind = next()", "use(kind)", "item = next()", "use(item)"] {
        assert!(codes.iter().any(|c| c == code), "{code} missing from CFG");
    }
    assert!(codes.iter().any(|c| c == "use(value)"));
}

#[test]
fn joern_drops_while_and_switch_with_declaration_conditions() {
    let codes = cfg_codes(true);
    for code in ["kind = next()", "use(kind)", "item = next()", "use(item)"] {
        assert!(!codes.iter().any(|c| c == code), "{code} kept in CFG");
    }
    assert!(codes.iter().any(|c| c == "value = next()"));
    assert!(codes.iter().any(|c| c == "use(value)"));
}
