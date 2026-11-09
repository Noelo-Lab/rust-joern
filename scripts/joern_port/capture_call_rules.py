#!/usr/bin/env python3
"""Freeze exact upstream function bodies used by the direct Rust call port.

This extracts and audits the port input; it does not pretend to transpile CDT
bindings. Scala comments/strings use the same bounded lexer as test import.
"""

import argparse
import hashlib
import json
from pathlib import Path
import subprocess

from import_fixtures import COMMIT, tokens

ROOT = Path(__file__).resolve().parents[2]
OWNERS = {
    "joern-cli/frontends/c2cpg/src/main/scala/io/joern/c2cpg/astcreation/AstForExpressionsCreator.scala": [
        "astForCppCallExpression",
        "astForCppCallExpressionUntyped",
        "createPointerCallAst",
        "createCFunctionCallAst",
        "astForCCallExpressionUntyped",
    ],
    "joern-cli/frontends/c2cpg/src/main/scala/io/joern/c2cpg/astcreation/AstCreatorHelper.scala": [
        "createCallAst"
    ],
    "joern-cli/frontends/x2cpg/src/main/scala/io/joern/x2cpg/AstCreatorBase.scala": [
        "setArgumentIndices"
    ],
}


def extract(source, name):
    ts = tokens(source)
    matches = [
        i for i in range(len(ts) - 1) if ts[i].text == "def" and ts[i + 1].text == name
    ]
    if len(matches) != 1:
        raise ValueError(f"ambiguous/missing upstream function {name}")
    index = matches[0]
    start = ts[index].start
    opening = next(
        i
        for i in range(index + 2, len(ts))
        if ts[i].kind == "punct" and ts[i].text == "{"
    )
    depth = 0
    for token in ts[opening:]:
        if token.kind != "punct":
            continue
        if token.text == "{":
            depth += 1
        elif token.text == "}":
            depth -= 1
            if depth == 0:
                return source[start : token.end], source.count("\n", 0, start) + 1
    raise ValueError(f"unclosed function body {name}")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--source", type=Path, required=True)
    parser.add_argument(
        "--output",
        type=Path,
        default=ROOT / "tests/reference/joern-4.0.150/call-rules.json",
    )
    parser.add_argument("--check", action="store_true")
    args = parser.parse_args()
    source = args.source.resolve(strict=True)
    head = subprocess.check_output(
        ["git", "-C", str(source), "rev-parse", "HEAD"], text=True
    ).strip()
    if head != COMMIT:
        raise ValueError("wrong upstream source commit")
    rules = []
    for path, names in OWNERS.items():
        data = (source / path).read_bytes()
        committed = subprocess.check_output(
            ["git", "-C", str(source), "show", "HEAD:" + path]
        )
        if committed != data:
            raise ValueError("modified upstream port source")
        for name in names:
            body, line = extract(data.decode(), name)
            rules.append(
                {
                    "owner": path,
                    "source_sha256": hashlib.sha256(data).hexdigest(),
                    "function": name,
                    "line": line,
                    "scala": body,
                    "fragment_sha256": hashlib.sha256(body.encode()).hexdigest(),
                }
            )
    result = {
        "schema": "joern-call-port-source-v1",
        "upstream": "https://github.com/joernio/joern",
        "commit": COMMIT,
        "license": "Apache-2.0 (see LICENSE)",
        "bounds": "Exact direct-port source, not generated Rust and not CDT type resolution",
        "functions": rules,
    }
    encoded = json.dumps(result, indent=2) + "\n"
    if args.check:
        if args.output.read_text() != encoded:
            raise ValueError("call port source snapshot is stale")
    else:
        args.output.write_text(encoded)
    print(
        json.dumps(
            {
                "commit": COMMIT,
                "functions": len(rules),
                "status": "verified" if args.check else "captured",
            }
        )
    )


if __name__ == "__main__":
    main()
