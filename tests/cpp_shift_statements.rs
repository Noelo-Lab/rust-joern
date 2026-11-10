use rust_joern::{analyze, graph::Options};

#[test]
fn shift_statements_stay_executable_after_type_evidence() {
    let source = r#"
namespace ns {
    struct Tag {};
    template<class T> struct Box {};
    unsigned input;
}
typedef unsigned Number;
void emit(ns::Tag tag, Number Number) {
    ns::input << Number;
    ns::input >> Number;
    Number << ns::input;
    Number >> ns::input;
    ns::Box<ns::Box<int>> nested;
}
"#;
    let result = analyze(
        source,
        "shift.cpp",
        &Options {
            language: Some("cpp".into()),
            strict: true,
            ..Options::default()
        },
    )
    .unwrap();
    let emit = result.functions.iter().find(|f| f.name == "emit").unwrap();
    let shifts: Vec<_> = emit
        .cpg
        .nodes
        .iter()
        .filter(|node| {
            matches!(
                node.name.as_deref(),
                Some("<operator>.shiftLeft" | "<operator>.arithmeticShiftRight")
            )
        })
        .collect();
    assert_eq!(
        shifts
            .iter()
            .map(|node| node.code.as_str())
            .collect::<Vec<_>>(),
        [
            "ns::input << Number",
            "ns::input >> Number",
            "Number << ns::input",
            "Number >> ns::input",
        ]
    );
    for shift in shifts {
        assert!(
            emit.cpg
                .edges
                .iter()
                .any(|edge| { edge.kind == "CFG" && edge.target == shift.id })
        );
    }
    assert!(
        emit.cpg
            .nodes
            .iter()
            .any(|node| { node.kind == "LOCAL" && node.name.as_deref() == Some("nested") })
    );
}
