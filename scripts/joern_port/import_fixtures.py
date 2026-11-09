#!/usr/bin/env python3
"""Import literal upstream Scala test inputs, without executing Scala.

This is source-only qualification, NOT a port of Scala assertions or fixture
configuration. Every code(...) call is accounted for; composed/interpolated
sources and nonliteral filenames are reported separately, never guessed.
"""

from __future__ import annotations

import argparse
from dataclasses import dataclass
import hashlib
import json
from pathlib import Path
import re
import subprocess

COMMIT = "958fdd3d976197a783f8ade1254b43e977648f28"


@dataclass
class Token:
    kind: str
    text: str
    start: int
    end: int


def tokens(source: str) -> list[Token]:
    result = []
    position = 0
    while position < len(source):
        start = position
        if source[position].isspace():
            position += 1
        elif source.startswith("//", position):
            end = source.find("\n", position)
            position = len(source) if end < 0 else end
        elif source.startswith("/*", position):
            position += 2
            depth = 1
            while depth:
                if position >= len(source):
                    raise ValueError("unclosed Scala comment")
                if source.startswith("/*", position):
                    depth += 1
                    position += 2
                elif source.startswith("*/", position):
                    depth -= 1
                    position += 2
                else:
                    position += 1
        elif source.startswith('"""', position):
            end = source.find('"""', position + 3)
            if end < 0:
                raise ValueError("unclosed Scala multiline string")
            position = end + 3
            result.append(Token("triple", source[start + 3 : end], start, position))
        elif source[position] in "\"'":
            quote = source[position]
            position += 1
            while position < len(source) and source[position] != quote:
                position += 2 if source[position] == "\\" else 1
            if position >= len(source):
                raise ValueError("unclosed Scala string/character")
            position += 1
            result.append(
                Token(
                    "string" if quote == '"' else "char",
                    source[start:position],
                    start,
                    position,
                )
            )
        else:
            match = re.match(r"[\w$]+", source[position:])
            position += len(match[0]) if match else 1
            result.append(
                Token(
                    "word" if match else "punct",
                    source[start:position],
                    start,
                    position,
                )
            )
    return result


def literal(token: Token) -> str:
    if token.kind == "triple":
        # Scala performs Unicode escapes even in triple strings. Do not guess.
        if re.search(r"\\u+[0-9a-fA-F]{4}", token.text):
            raise ValueError("Scala Unicode processing requires explicit review")
        return token.text
    if token.kind == "string":
        try:
            return json.loads(token.text)
        except ValueError as error:
            raise ValueError("unsupported Scala string escape") from error
    raise ValueError("source is not a standalone literal")


def extract(source: str) -> list[dict]:
    ts = tokens(source)
    result = []
    for index, token in enumerate(ts[:-1]):
        if token.text != "code" or ts[index + 1].text != "(":
            continue
        row = {"line": source.count("\n", 0, token.start) + 1}
        try:
            cursor = index + 2
            value = literal(ts[cursor])
            cursor += 1
            if [t.text for t in ts[cursor : cursor + 2]] == [".", "stripMargin"]:
                # Scala stripMargin removes leading <= U+0020 through '|',
                # preserving line endings, empty lines and all other bytes.
                value = re.sub(r"(?m)^[\x00-\x09\x0b-\x0c\x0e-\x20]*\|", "", value)
                cursor += 2
            filename = "input.c"  # C2CpgSuite/AstC2CpgSuite default C_EXT.
            if ts[cursor].text == ",":
                filename = literal(ts[cursor + 1])
                cursor += 2
            if ts[cursor].text != ")":
                raise ValueError(
                    "composed source, custom stripMargin or unsupported arguments"
                )
            if not filename.endswith((".c", ".cpp", ".cc", ".cxx", ".C")):
                raise ValueError("unknown explicit filename language")
            # Nondefault suite configuration cannot silently acquire C defaults.
            suite = re.search(r"extends (?:Ast)?C2CpgSuite\s*\(", source)
            if suite:
                raise ValueError("parameterized fixture configuration requires review")
            row.update(
                status="literal_source_only",
                text=value,
                filename=filename,
                language="c" if filename.endswith(".c") else "cpp",
                sha256=hashlib.sha256(value.encode()).hexdigest(),
            )
        except (ValueError, IndexError) as error:
            row.update(status="requires_review", reason=str(error))
        result.append(row)
    return result


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--source", type=Path, required=True, help="Pinned Joern checkout"
    )
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    checkout = args.source.resolve(strict=True)
    commit = subprocess.check_output(
        ["git", "-C", str(checkout), "rev-parse", "HEAD"], text=True
    ).strip()
    if commit != COMMIT:
        raise ValueError("upstream checkout is not pinned Joern 4.0.150")
    dirty = subprocess.check_output(
        ["git", "-C", str(checkout), "status", "--porcelain", "--untracked-files=no"],
        text=True,
    )
    if dirty:
        raise ValueError("upstream reference has local modifications")
    base = checkout / "joern-cli/frontends/c2cpg/src"
    audit, rows, files, modules = [], [], {}, []
    for path in sorted(base.rglob("*.scala")):
        relative = path.relative_to(checkout).as_posix()
        data = path.read_bytes()
        files[relative] = hashlib.sha256(data).hexdigest()
        source = data.decode()
        if "/main/" in relative:
            modules.append(
                {
                    "path": relative,
                    "lines": len(source.splitlines()),
                    "methods": re.findall(r"\bdef\s+(\w+)", source),
                    "cdt_references": sorted(
                        set(
                            re.findall(
                                r"\b(?:IAST|ICPPAST|ICPP|IBinding|IType)\w*", source
                            )
                        )
                    ),
                }
            )
        else:
            try:
                entries = extract(source)
            except ValueError as error:
                raise ValueError(
                    f"cannot inventory Scala file {relative}: {error}"
                ) from error
            for entry in entries:
                entry["path"] = relative
                audit.append(entry)
                if entry["status"] == "literal_source_only":
                    rows.append(
                        {
                            key: entry[key]
                            for key in ("text", "filename", "language", "sha256")
                        }
                        | {
                            "origins": [
                                {
                                    "upstream_commit": commit,
                                    "path": relative,
                                    "line": entry["line"],
                                    "kind": "upstream_test_source_control_not_corpus",
                                }
                            ]
                        }
                    )
    args.output.mkdir(parents=True, exist_ok=True)
    contract = {
        "schema": "joern-upstream-literal-import-v1",
        "upstream_commit": commit,
        "source_only": True,
        "scala_assertions_ported": False,
        "files": files,
    }
    (args.output / "INPUTS.json").write_text(
        json.dumps(contract | {"rows": rows}, indent=2) + "\n"
    )
    (args.output / "INVENTORY.json").write_text(
        json.dumps(contract | {"calls": audit, "modules": modules}, indent=2) + "\n"
    )
    print(
        json.dumps(
            {
                "literal_sources": len(rows),
                "review_required": len(audit) - len(rows),
                "main_modules": len(modules),
                "upstream_commit": commit,
            }
        )
    )


if __name__ == "__main__":
    main()
