import unittest

from rust_joern import parse_code


class DiagnosticSpanTests(unittest.TestCase):
    def test_lexer_range_is_in_original_utf8_bytes(self):
        prefix = "/* café */\r\nint f(void) { return 1; }\r\n".encode()
        comment = "/* unfinished\nnon-ASCII é".encode()
        source = prefix + comment
        original = parse_code(source)
        with_spans = parse_code(source, diagnostic_spans=True)

        self.assertNotIn("diagnostic_spans", original.raw)
        self.assertIs(with_spans.raw["diagnostic_spans"], True)
        self.assertEqual(len(with_spans.diagnostics), 1)
        span = with_spans.diagnostics[0]["span"]
        self.assertEqual(span, {"start_byte": len(prefix), "end_byte": len(source)})
        self.assertEqual(source[span["start_byte"]:span["end_byte"]], comment)

        comparable = dict(with_spans.raw)
        comparable.pop("diagnostic_spans")
        comparable["diagnostics"] = [
            {key: value for key, value in diagnostic.items() if key != "span"}
            for diagnostic in with_spans.diagnostics
        ]
        self.assertEqual(original.raw, comparable)

    def test_parser_and_builder_spans_preserve_strict_failure(self):
        source = b"int bad(int a) { co_await a; return a; } int good() { return 1; }"
        filename = "scope.cpp"
        result = parse_code(source, filename, diagnostic_spans=True)
        spans = {}
        for diagnostic in result.diagnostics:
            self.assertEqual(diagnostic["filename"], filename)
            span = diagnostic["span"]
            spans[diagnostic["message"]] = source[span["start_byte"]:span["end_byte"]]
        self.assertEqual(spans, {
            "coroutine control flow is unsupported": b"co_await a",
            "unsupported statement; CFG parity is unverified": b"co_await a;",
        })
        for diagnostic_spans in (False, True):
            with self.assertRaisesRegex(RuntimeError, "coroutine control flow is unsupported"):
                parse_code(source, filename, strict=True, diagnostic_spans=diagnostic_spans)

    def test_unlocated_warning_has_no_range(self):
        analysis = parse_code("int f(void) { return 1; }", data_flow=True, diagnostic_spans=True)
        self.assertTrue(analysis.diagnostics)
        for diagnostic in analysis.diagnostics:
            self.assertEqual(diagnostic["severity"], "warning")
            self.assertNotIn("span", diagnostic)
