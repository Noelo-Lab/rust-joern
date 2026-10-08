"""Opt-in DecBench compatibility: PYTHONPATH=compat:python.

This shim is deliberately excluded from the installed rust-joern package so the
original PyJoern remains available for reference comparisons. DecBench consumes
only CFGs, so this shim skips DDG generation and rejects unsupported or recovered
executable syntax by default. Explicit flags are preserved. The generic
rust_joern API retains permissive parsing with diagnostics and PyJoern's original
DDG default.
"""

from rust_joern import Function, parse_source as _parse_source

__all__ = ["Function", "parse_source"]


def parse_source(source_path, no_metadata=False, no_cfg=False, no_ddg=True, no_ast=False, is_decompilation=False, **options):
    options.setdefault("strict", True)
    options.setdefault("preprocessed", True)
    return _parse_source(source_path, no_metadata, no_cfg, no_ddg, no_ast, is_decompilation, **options)
