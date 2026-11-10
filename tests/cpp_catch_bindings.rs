use rust_joern::{analyze, graph::Options};

#[test]
fn named_catch_parameters_shadow_only_in_their_own_handler() {
    let source = r#"
struct Error {};
void risky();
void sink();
int f(int value) {
    try { risky(); }
    catch (double value) { (void)value; }
    catch (const Error& value) { (void)value; }
    catch (unsigned) { sink(); }
    catch (...) { sink(); }
    return value;
}
"#;
    let result = analyze(
        source,
        "catch.cpp",
        &Options {
            language: Some("cpp".into()),
            strict: true,
            ..Options::default()
        },
    )
    .unwrap();
    let f = result.functions.iter().find(|f| f.name == "f").unwrap();
    let nodes = &f.cpg.nodes;
    let edges = &f.cpg.edges;
    let outer = nodes
        .iter()
        .find(|node| node.kind == "METHOD_PARAMETER_IN" && node.name.as_deref() == Some("value"))
        .unwrap();
    let locals: Vec<_> = nodes.iter().filter(|node| node.kind == "LOCAL").collect();
    assert_eq!(locals.len(), 2, "unnamed handlers do not introduce a local");
    assert_eq!(locals[0].type_name.as_deref(), Some("double"));
    assert_eq!(locals[1].type_name.as_deref(), Some("Error&"));
    for local in &locals {
        assert!(edges.iter().any(|edge| {
            edge.kind == "AST"
                && edge.target == local.id
                && nodes[edge.source as usize].name.as_deref() == Some("CATCH")
        }));
        assert!(!edges.iter().any(|edge| {
            edge.kind == "CFG" && (edge.source == local.id || edge.target == local.id)
        }));
    }
    let uses: Vec<_> = nodes
        .iter()
        .filter(|node| node.kind == "IDENTIFIER" && node.name.as_deref() == Some("value"))
        .collect();
    assert_eq!(uses.len(), 3);
    for (usage, declaration) in uses.iter().zip([locals[0], locals[1], outer]) {
        assert_eq!(usage.type_name, declaration.type_name);
        let bindings: Vec<_> = edges
            .iter()
            .filter(|edge| edge.kind == "REF" && edge.source == usage.id)
            .map(|edge| edge.target)
            .collect();
        assert_eq!(bindings, [declaration.id]);
    }
}
