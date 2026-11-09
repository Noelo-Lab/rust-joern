use rust_joern::{analyze, graph::Options};

fn options() -> Options {
    Options {
        language: Some("cpp".into()),
        strict: true,
        ..Options::default()
    }
}

#[test]
fn range_for_init_runs_once_and_its_binding_ends_at_the_loop() {
    let source = "struct Range { int *begin(); int *end(); };
        int start(); Range values(int); void visit(int, int);
        int probe() {
            int i = 99;
            for (int i = start(); const auto& x : values(i)) {
                if (x == 1) continue;
                if (x == 3) break;
                visit(i, x);
            }
            return i;
        }";
    let result = analyze(source, "range.cpp", &options()).unwrap();
    let function = result.functions.iter().find(|f| f.name == "probe").unwrap();
    let graph = &function.cpg;
    let node = |code: &str| graph.nodes.iter().find(|n| n.code == code).unwrap().id;
    let successors = |id| {
        graph
            .edges
            .iter()
            .filter(move |edge| edge.kind == "CFG" && edge.source == id)
            .map(|edge| edge.target)
            .collect::<Vec<_>>()
    };
    let init = node("i = start()");
    let values = node("values(i)");
    let test = node("<range-for test>");
    let advance = node("<range-for advance>");
    assert_eq!(successors(node("start()")), [init]);
    assert!(
        successors(init)
            .iter()
            .any(|&argument| successors(argument) == [values])
    );
    assert_eq!(successors(values), [test]);
    assert_eq!(successors(node("continue;")), [advance]);
    assert_eq!(successors(advance), [test]);
    let after = successors(node("break;"))[0];
    assert!(
        successors(test).contains(&after),
        "empty range exits without visiting"
    );
    assert_eq!(successors(after), [node("return i;")]);

    let locals: Vec<_> = graph
        .nodes
        .iter()
        .filter(|n| n.kind == "LOCAL" && n.name.as_deref() == Some("i"))
        .map(|n| n.id)
        .collect();
    assert_eq!(locals.len(), 2);
    let binding = |parent| {
        let argument = graph
            .edges
            .iter()
            .find(|e| {
                e.source == parent
                    && e.kind == "ARGUMENT"
                    && graph.nodes[e.target as usize].code == "i"
            })
            .unwrap()
            .target;
        graph
            .edges
            .iter()
            .find(|e| e.source == argument && e.kind == "REF")
            .unwrap()
            .target
    };
    assert_eq!(binding(values), locals[1]);
    assert_eq!(binding(node("visit(i, x)")), locals[1]);
    assert_eq!(binding(node("return i;")), locals[0]);
}

#[test]
fn range_for_accepts_expression_and_empty_init_statements() {
    let source = "void setup(); void visit(int); void probe(bool choose) {
        int values[2] = {1, 2}, other[2] = {3, 4};
        for (setup(); int x : choose ? values : other) visit(x);
        for (; int x : values) visit(x);
    }";
    let result = analyze(source, "range.cpp", &options()).unwrap();
    let function = result.functions.iter().find(|f| f.name == "probe").unwrap();
    assert_eq!(
        function
            .cpg
            .nodes
            .iter()
            .filter(|n| n.name.as_deref() == Some("FOR"))
            .count(),
        2
    );
    let setup = function
        .cpg
        .nodes
        .iter()
        .find(|n| n.code == "setup()")
        .unwrap();
    assert!(
        function
            .cfg
            .nodes
            .iter()
            .any(|block| block.statements.contains(&setup.id))
    );
    assert!(function.cpg.nodes.iter().any(|node| {
        node.name.as_deref() == Some("<operator>.conditional")
            && node.code == "choose ? values : other"
    }));
}
