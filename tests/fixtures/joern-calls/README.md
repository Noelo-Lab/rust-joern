# Bounded original-Joern call AST contracts

These are **source controls**, not benchmark corpus programs. `calls.cpp`
intentionally includes undeclared names/types to exercise c2cpg's unresolved
call path on decompiler-like text. The whole file is not claimed to compile
or execute as valid C++. Other rows exercise declared members, explicit
virtual methods, pointers, lambdas, destructor calls and short circuiting.
`calls.c` independently checks the C call paths.

The original is unchanged Joern v4.0.150 bundled with PyJoern 4.0.150.4.
Capture from an isolated working directory using the original installation:

```sh
env -u PYTHONPATH /path/to/original/joern-cli/joern \
  --script /path/to/repo/tests/fixtures/ddg-lambdas/capture.sc \
  --param target_dir=/path/to/repo/tests/fixtures/joern-calls/calls.cpp > cpp.log
python3 /path/to/repo/scripts/joern_port/call_contract.py \
  --log cpp.log --source /path/to/repo/tests/fixtures/joern-calls/calls.cpp \
  --output /path/to/repo/tests/fixtures/joern-calls/cpp-original.json
```

Repeat independently for `calls.c`. The script does not run the replacement
backend to generate expected results. Raw original logs and intermediate
candidate failures remain in `workspace/joern-call-port-2026-10-25/original/`.
The frozen contracts retain source, raw-log, capture-script and projector
hashes. The reference source/JVM identity can be independently rechecked with
`scripts/joern_port/verify_reference.py`.

The projection checks **18 C++ methods / 25 calls** and **5 C methods / 7
calls**, including prototypes and a detached lambda method. Method and call
populations are preserved; duplicate identities are errors. Every recorded
call checks name/fullname, source location, `ARGUMENT` edge indices, receiver
identity, shared base/receiver identity and the multiset of direct AST
children. Identically spelled duplicated receiver nodes do not count as a
shared AST. Receiver-only roots have no argument-zero edge.

This is not full CPG equality: operator expressions other than pointerCall,
type inference, dispatch/signature properties, raw node argumentIndex/order
properties, DDG and execution semantics are outside the projection. CFG and
native effect checks run separately; parsing success does not prove valid
C++ output. Existing qualified/inherited/overloaded member lookup limitations
stay explicit and do not acquire guessed receiver edges.
