import unittest

import rust_joern


class CppSourceContextTest(unittest.TestCase):
    def test_transport_acknowledges_names(self):
        result = rust_joern.parse_code(
            "int consume(); int f(){return consume(Options{});}",
            filename="x.cpp", language="cpp", preprocessed=True, strict=True,
            cpp_type_names=("Options",),
        )
        self.assertEqual(result.raw["cpp_type_names"], ["Options"])

    def test_rejects_non_sequence_and_non_string(self):
        for names in ("Options", [1]):
            with self.assertRaises(TypeError):
                rust_joern.parse_code("int f(){return 0;}", cpp_type_names=names)
