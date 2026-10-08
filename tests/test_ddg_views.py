"""The public PyJoern DDG and the native REACHING_DEF graph have distinct contracts.

Statement expectations were checked against unchanged PyJoern 4.0.150.4's
JIL lifter, including its unsupported-statement and operand parsing behavior.
"""

import unittest

import networkx as nx

import rust_joern


def node(id_, kind, code, line=3, **properties):
    return {"id": id_, "kind": kind, "code": code, "line": line, **properties}


def edge(source, target, label=None, kind="REACHING_DEF"):
    result = {"source": source, "target": target, "kind": kind}
    if label is not None:
        result["label"] = label
    return result


def copy_function_data():
    nodes = [
        node(1, "METHOD", "int copy(int x) { int y = x; return y; }", name="copy"),
        node(2, "METHOD_RETURN", "RET", type_name="int"),
        node(3, "METHOD_PARAMETER_IN", "int x", name="x", type_name="int"),
        node(4, "CALL", "y = x", name="<operator>.assignment"),
        node(5, "RETURN", "return y;"),
        node(6, "IDENTIFIER", "y", name="y"),
        node(7, "LITERAL", "1"),
    ]
    edges = [
        edge(5, 2, "<RET>"), edge(4, 2, "y"), edge(4, 2, "x"), edge(4, 2, "y = x"),
        edge(1, 3), edge(3, 4, "x"), edge(1, 4), edge(6, 5, "y"), edge(4, 6, "y"), edge(1, 6),
    ]
    ast = [edge(1, 3, kind="AST"), edge(1, 4, kind="AST"), edge(1, 5, kind="AST"), edge(5, 6, kind="AST")]
    return {
        "name": "copy", "fullname": "copy", "filename": "flow.c", "return_type": "int",
        "signature": "int(int)", "start_line": 3, "end_line": 3,
        "cpg": {"nodes": nodes, "edges": ast}, "cfg": {"nodes": [], "edges": []},
        "ddg": {"nodes": nodes, "edges": edges},
        "ddg_view": {"nodes": nodes + [nodes[3]], "edges": edges},
    }


class DdgViewTests(unittest.TestCase):
    def test_public_ddg_lifts_blocks_and_raw_ddg_preserves_properties(self):
        function = rust_joern.Function(copy_function_data())
        graph = function.ddg
        self.assertIs(type(graph), nx.DiGraph)
        self.assertIs(type(function.ddg_raw), nx.MultiDiGraph)
        self.assertEqual((graph.name, len(graph), graph.number_of_edges()), ("copy", 6, 8))
        self.assertEqual((len(function.ddg_raw), function.ddg_raw.number_of_edges()), (7, 10))
        blocks = {block.statements[0].id: block for block in graph}
        self.assertEqual({id_: block.idx for id_, block in blocks.items()}, dict(zip(range(1, 7), range(6))))
        self.assertTrue(all(block.addr == 3 for block in graph))
        self.assertTrue(blocks[1].is_entrypoint)
        self.assertTrue(blocks[2].is_exitpoint)
        self.assertEqual(graph.out_degree(blocks[2]), 0)
        self.assertEqual(str(blocks[4]), "3.3:\ny = x\n")
        self.assertIsInstance(blocks[3].statements[0], rust_joern.Parameter)
        self.assertIsInstance(blocks[4].statements[0], rust_joern.Assignment)
        self.assertIsInstance(blocks[5].statements[0], rust_joern.Return)
        self.assertIsInstance(blocks[6].statements[0], rust_joern.UnsupportedStmt)
        for block, attrs in graph.nodes(data=True):
            self.assertEqual(attrs, {"node": block})
        for source, target, attrs in graph.edges(data=True):
            self.assertEqual(attrs, {"src": source, "dst": target})
        self.assertEqual({attrs["label"] for attrs in function.ddg_raw[4][2].values()}, {"x", "y", "y = x"})
        self.assertEqual(function.ddg_raw.nodes[6]["code"], "y")
        self.assertIs(function.ddg, graph)
        self.assertIs(function.ddg_raw, function.ddg_raw)

    def test_disabled_and_absent_data_flow_are_none_but_empty_view_is_a_graph(self):
        data = copy_function_data()
        disabled = rust_joern.Function(data, no_ddg=True)
        self.assertIsNone(disabled.ddg)
        self.assertIsNone(disabled.ddg_raw)
        del data["ddg"]
        del data["ddg_view"]
        absent = rust_joern.Function(data)
        self.assertIsNone(absent.ddg)
        self.assertIsNone(absent.ddg_raw)
        data["ddg_view"] = {"nodes": [], "edges": []}
        self.assertIs(type(rust_joern.Function(data).ddg), nx.DiGraph)
        self.assertEqual(len(rust_joern.Function(data).ddg), 0)

    def test_dot_labels_include_parent_expression_and_html4_escaping(self):
        data = copy_function_data()
        self.assertEqual(
            rust_joern.Function._ddg_labels(data["ddg_view"], data["cpg"])[6],
            "(IDENTIFIER,y,return y;)<SUB>3</SUB>",
        )
        identifier = node(10, "IDENTIFIER", "b")
        access = node(11, "CALL", "b->field", name="<operator>.indirectFieldAccess")
        ret = node(12, "RETURN", "return b->field;")
        cpg = {"nodes": [identifier, access, ret], "edges": [edge(11, 10, kind="AST"), edge(12, 11, kind="AST")]}
        self.assertEqual(rust_joern._ddg_label(identifier, cpg), "(IDENTIFIER,b,return b-&gt;field;)<SUB>3</SUB>")
        condition = node(15, "CONTROL_STRUCTURE", "if (b)", name="IF")
        cpg = {"nodes": [identifier, condition], "edges": [edge(15, 10, kind="AST")]}
        self.assertEqual(rust_joern._ddg_label(identifier, cpg), "(IDENTIFIER,b,if (b))<SUB>3</SUB>")
        literal = node(16, "LITERAL", "1")
        condition = node(17, "CONTROL_STRUCTURE", "while (1)", name="WHILE")
        cpg = {"nodes": [literal, condition], "edges": [edge(17, 16, kind="AST")]}
        self.assertEqual(rust_joern._ddg_label(literal, cpg), "(LITERAL,1,while (1))<SUB>3</SUB>")
        quoted = node(13, "CALL", 'say("µ & < >", \'x\')', name="say")
        self.assertEqual(rust_joern._ddg_label(quoted), '(say,say(&quot;&micro; &amp; &lt; &gt;&quot;, \'x\'))<SUB>3</SUB>')
        self.assertEqual(rust_joern._ddg_limit("a" * 51), "a" * 47 + "...")
        self.assertEqual(rust_joern._ddg_limit("a" * 50), "a" * 50)
        self.assertEqual(rust_joern._ddg_limit("😀" * 23 + "abcde"), "😀" * 23 + "a...")
        long_parameter_code = "int " + "parameter_name_" * 5
        self.assertEqual(
            rust_joern._ddg_label(node(14, "METHOD_PARAMETER_IN", long_parameter_code)),
            f"(PARAM,{long_parameter_code})<SUB>3</SUB>",
        )

    def test_missing_source_line_omits_sub_but_negative_line_is_preserved(self):
        missing = node(1, "METHOD_RETURN", "RET", line=0, type_name="int")
        negative = node(2, "METHOD_PARAMETER_IN", "int x", line=-1)
        data = {"nodes": [missing, negative], "edges": [edge(2, 1)]}
        self.assertEqual(rust_joern._ddg_label(missing), "(METHOD_RETURN,int)")
        self.assertEqual(rust_joern._ddg_label(negative), "(PARAM,int x)<SUB>-1</SUB>")
        graph = rust_joern.Function._ddg(data, data)
        blocks = {block.statements[0].id: block for block in graph}
        self.assertFalse(blocks[1].is_exitpoint)
        self.assertIsInstance(blocks[1].statements[0], rust_joern.UnsupportedStmt)
        self.assertIsInstance(blocks[2].statements[0], rust_joern.Parameter)
        self.assertTrue(all(block.addr is None for block in graph))

    def test_omitted_expression_code_uses_generated_accessor_default_only_in_ddg(self):
        # Joern's generated accessor returns <empty> for omitted CODE, which
        # its propertiesMap/capture normalize to "". These are original DOT
        # labels from nested_empty and the transparent __offsetof__ expansion.
        block = node(1, "BLOCK", "", line=14)
        identifier = node(2, "IDENTIFIER", "x", line=5)
        expansion = node(3, "BLOCK", "", line=5)
        literal = node(4, "LITERAL", '""', line=5)
        cpg = {"nodes": [block, identifier, expansion, literal], "edges": [edge(3, 2, kind="AST")]}
        view = {"nodes": [block, identifier, literal], "edges": [edge(1, 2), edge(2, 4)]}
        self.assertEqual(rust_joern.Function._ddg_labels(view, cpg), {
            1: "(BLOCK,&lt;empty&gt;,&lt;empty&gt;)<SUB>14</SUB>",
            2: "(IDENTIFIER,x,&lt;empty&gt;)<SUB>5</SUB>",
            4: "(LITERAL,&quot;&quot;,&quot;&quot;)<SUB>5</SUB>",
        })
        graph = rust_joern.Function._ddg(view, cpg)
        statements = {statement.id: statement for node_ in graph for statement in node_.statements}
        self.assertEqual(statements[1].raw_text, "BLOCK,&lt;empty&gt;,&lt;empty&gt;")
        self.assertEqual(statements[2].raw_text, "IDENTIFIER,x,&lt;empty&gt;")
        self.assertEqual(block["code"], "")
        self.assertEqual(expansion["code"], "")
        self.assertEqual(literal["code"], '""')

    def test_explicit_empty_identifier_code_is_preserved(self):
        # Original fp_only_mixed_second has IDENTIFIER name p, CODE="" with
        # code_property_present=true, and no source location. The nested
        # pointer declarator's empty IASTName supplies that explicit value.
        identifier = node(1, "IDENTIFIER", "", line=0, name="p")
        assignment = node(2, "CALL", "(*p)(int) = x ? target : second", line=8, name="<operator>.assignment")
        cpg = {"nodes": [identifier, assignment], "edges": [edge(2, 1, kind="AST")]}
        self.assertEqual(rust_joern._ddg_code(identifier), "")
        self.assertEqual(
            rust_joern._ddg_label(identifier, cpg),
            "(IDENTIFIER,,(*p)(int) = x ? target : second)",
        )
        self.assertEqual(identifier["code"], "")

    def test_jil_statement_types_operands_and_fallback_match_pyjoern(self):
        cases = [
            ("(&lt;operator&gt;.assignment,y = x)<SUB>3</SUB>", "CALL", "Assignment", "y = x",
             {"raw_text": ["assignment", "y=x"], "source_line_number": 3, "src": "y", "dst": "x"}),
            ("(RETURN,return y;,return y;)<SUB>3</SUB>", "RETURN", "Return", "return y;,returny;",
             {"raw_text": ["", "returny;", "returny;"], "source_line_number": 3, "ret": "y;,returny;"}),
            ("(IDENTIFIER,y,return y;)<SUB>3</SUB>", "IDENTIFIER", "UnsupportedStmt", "<UnsupportedStmt: IDENTIFIER,y,returny;>",
             {"raw_text": "IDENTIFIER,y,returny;", "source_line_number": 3}),
            ("(PARAM,int x)<SUB>3</SUB>", "METHOD_PARAMETER_IN", "Parameter", "int x",
             {"raw_text": "PARAM,int x", "source_line_number": 3, "name": "x", "type": "int"}),
            ("(PARAM,struct Box *b)<SUB>7</SUB>", "METHOD_PARAMETER_IN", "UnsupportedStmt", "<UnsupportedStmt: (PARAM,struct Box *b)<SUB>7</SUB>>",
             {"raw_text": "(PARAM,struct Box *b)<SUB>7</SUB>", "source_line_number": None}),
            ("(&lt;operator&gt;.greaterThan,x &gt; 0)<SUB>4</SUB>", "CALL", "Compare", "x > 0",
             {"raw_text": ["greaterThan", "x&gt;0"], "source_line_number": 4, "type": 2, "arg1": "x", "arg2": "0"}),
            ("(&lt;operator&gt;.addition,x+1)<SUB>5</SUB>", "CALL", "UnsupportedStmt", "<UnsupportedStmt: ['addition', 'x+1']>",
             {"raw_text": ["addition", "x+1"], "source_line_number": 5}),
            ("(sink,sink(x,y))<SUB>6</SUB>", "CALL", "Call", "sink()",
             {"raw_text": "sink,sink(x,y)", "source_line_number": 6, "func": "sink", "args": [""]}),
            ("(&lt;operator&gt;.conditional,x ? y : z)<SUB>7</SUB>", "CALL", "Ternary", "x ? x : ?",
             {"raw_text": "&lt;operator&gt;.conditional,x?y:z", "source_line_number": 7, "cond": "x", "true": "x", "false": "?"}),
            ("(&lt;operator&gt;.logicalAnd,x &amp;&amp; y)<SUB>8</SUB>", "CALL", "BinOp", "x&amp;&amp;y && ",
             {"raw_text": ["logicalAnd", "x&amp;&amp;y"], "source_line_number": 8, "type": 2, "arg1": "x&amp;&amp;y", "arg2": ""}),
            ("(METHOD_REF,sink,sink)<SUB>9</SUB>", "METHOD_REF", "Nop", "FUNCTION_START",
             {"raw_text": "METHOD_REF,sink,sink", "source_line_number": 9, "type": 0}),
            ("(METHOD,f)", "METHOD", "UnsupportedStmt", "<UnsupportedStmt: >",
             {"raw_text": "", "source_line_number": None}),
        ]
        from unittest.mock import patch
        for label, kind, cls, text, expected in cases:
            with self.subTest(label=label):
                data = {"nodes": [node(1, kind, "")], "edges": [edge(1, 1)]}
                with patch.object(rust_joern.Function, "_ddg_labels", return_value={1: label}):
                    graph = rust_joern.Function._ddg(data, {})
                block, = graph
                statement, = block.statements
                self.assertEqual(type(statement).__name__, cls)
                self.assertEqual(str(statement), text)
                self.assertEqual({key: getattr(statement, key) for key in expected}, expected)
                self.assertEqual(block.addr, expected["source_line_number"])

    def test_multiline_dot_label_lifts_one_statement_per_line(self):
        multiline = node(1, "RETURN", "return x+\ny;", line=4)
        data = {"nodes": [multiline], "edges": [edge(1, 1)]}
        block, = rust_joern.Function._ddg(data, {"nodes": [multiline], "edges": []})
        self.assertIsNone(block.addr)
        self.assertEqual([statement.source_line_number for statement in block.statements], [None, None, 4])
        self.assertTrue(all(isinstance(statement, rust_joern.UnsupportedStmt) for statement in block.statements))

    def test_native_public_and_raw_ddg_are_both_available(self):
        function, = rust_joern.parse_code("int f(int x) { int y = x; return y; }", data_flow=True).functions
        self.assertIs(type(function.ddg), nx.DiGraph)
        self.assertIs(type(function.ddg_raw), nx.MultiDiGraph)
        self.assertTrue(function.ddg.number_of_edges())
        self.assertTrue(all(isinstance(block, rust_joern.Block) for block in function.ddg))
        self.assertIn("ddg_view", function.raw)


if __name__ == "__main__":
    unittest.main()
