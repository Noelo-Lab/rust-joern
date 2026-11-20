//! Decompilers print C++ names into C output. CDT's C parser fails on them and
//! Joern has no method; by default the C parser recovers the definitions.
use rust_joern::{analyze, graph::Options};

fn options(joern_cdt_failures: bool) -> Options {
    Options {
        language: Some("c".into()),
        preprocessed: true,
        strict: true,
        joern_cdt_failures,
        ..Options::default()
    }
}

fn names(source: &str) -> Vec<String> {
    analyze(source, "decompiled.c", &options(false))
        .unwrap_or_else(|error| panic!("{source}: {error}"))
        .functions
        .into_iter()
        .map(|function| function.name)
        .collect()
}

#[test]
fn template_id_definitions_are_named_without_arguments() {
    assert_eq!(
        names(
            "unsigned int BitSize<short unsigned int>(unsigned long a0)\n{\n  if (a0) return 16;\n  return 0;\n}"
        ),
        ["BitSize"]
    );
    assert_eq!(
        names(
            "unsigned int Uncompress<snappy::Writer<snappy::Allocator> >(long a0)\n{\n  return 0;\n}"
        ),
        ["Uncompress"]
    );
    assert_eq!(
        names("void StringTo<double>\n               (undefined8 param_1)\n\n{\n  return;\n}"),
        ["StringTo"]
    );
    let analysis = analyze(
        "char * snappy::EmitCopy<true>(char *op)\n{\n  while (*op) op++;\n  return op;\n}",
        "decompiled.c",
        &options(false),
    )
    .unwrap();
    let [method] = analysis.functions.as_slice() else {
        panic!("{:?}", analysis.functions);
    };
    assert_eq!(method.name, "EmitCopy");
    assert!(method.cfg.edges.len() >= 3);
}

#[test]
fn qualified_record_and_operator_new_calls_parse() {
    assert_eq!(
        names(
            "class std::_Rb_tree_node_base {\n} std::_Rb_tree_node_base;\n\nint f(void)\n{\n  return 0;\n}"
        ),
        ["f"]
    );
    let analysis = analyze(
        "void *g(unsigned long n)\n{\n  void *p = operator new[](n);\n  operator delete(p);\n  return operator new(n);\n}",
        "decompiled.c",
        &options(false),
    )
    .unwrap();
    let calls: Vec<_> = analysis.functions[0]
        .cpg
        .nodes
        .iter()
        .filter(|node| node.kind == "CALL")
        .filter_map(|node| node.name.as_deref())
        .filter(|name| name.starts_with("operator"))
        .collect();
    assert_eq!(calls, ["operator new[]", "operator delete", "operator new"]);
}

#[test]
fn c_comparisons_are_not_template_ids() {
    assert_eq!(
        names("int g(int a, int b, int c)\n{\n  return a < b > (c);\n}"),
        ["g"]
    );
}

#[test]
fn joern_cdt_failures_keeps_the_original_problems() {
    for source in [
        "unsigned int BitSize<short unsigned int>(unsigned long a0)\n{\n  return 0;\n}",
        "class std::_Rb_tree_node_base {\n} std::_Rb_tree_node_base;",
        "int a::F<b>(long x)\n{\n  return 0;\n}",
    ] {
        assert!(
            analyze(source, "decompiled.c", &options(true)).is_err(),
            "{source}"
        );
    }
    let call = analyze(
        "void *g(unsigned long n)\n{\n  return operator new[](n);\n}",
        "decompiled.c",
        &options(true),
    )
    .unwrap();
    assert!(
        call.functions[0]
            .cpg
            .nodes
            .iter()
            .all(|node| node.name.as_deref() != Some("operator new[]"))
    );
}

#[test]
fn qualified_parameter_types_remain_problems() {
    // Only the function's own name may carry `::`; the C corpus keeps
    // Joern's problem declaration for a qualified parameter type.
    assert_eq!(
        names("int a::F<b>(long x)\n{\n  return 0;\n}\nint q(NS::T v)\n{\n  return 1;\n}"),
        ["F"]
    );
}
