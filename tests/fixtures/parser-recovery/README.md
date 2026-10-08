These isolated C/C++ translation units were parsed with original PyJoern
4.0.150.4 and Joern v4.0.150. Each unchanged `.pyjoern.json` records the input
hash, language through the file extension, complete function coverage,
directed CFG edges, entry/exit roles, degeneracy, and original generation time.
The sources exercise general syntax from the DecBench failures; no dataset
function names participate in the implementation.

- Microsoft assembly is a CDT problem statement. Its closing brace ends the
  current enclosing compound; earlier executable statements remain. Following
  methods still appear in the translation unit.
- Unknown parenthesized identifiers remain calls when an expression can follow
  them. A following cast type-id disambiguates a chain of casts.
- C qualification syntax produces a problem expression in conditions and a
  dropped problem statement in assignment/return/declaration contexts. Invalid
  `for` header slots discard the loop while retaining later valid header
  expressions as sibling statements. A label on that discarded loop also
  disappears; the recovered header expressions stay outside the label. A GNU inner statement-expression retains
  its outer construct when an inner statement is invalid.
- A bad scalar `if`/`while` body drops its complete control statement, while
  braced bodies keep their outer control structure. A bad scalar `do` body
  recovers the trailing `while` as a separate statement.
- GNU builtin macros retain their call and an expanded path, with a bypass
  edge. Original argument cloning follows the expansion's formatted CODE.
  Synthetic macro methods are included in the coverage comparison.
- CDT's C parser drops bracket attribute problem statements. The C++ parser
  accepts their annotations and keeps the executable statement that follows.
  Standalone GNU attributes produce no executable node in either language;
  in scalar controls they are problem statements in C and empty bodies in C++.
- Bare type arguments to ordinary calls are syntax problems. CDT drops their
  enclosing statements or declaration initializers, and keeps an UNKNOWN
  condition. Casts, `sizeof`, identifier arguments, and the predefined compiler
  macros remain valid. C++ also retains a recovered prototype when the malformed
  condition has a reference declarator shape.
- Decorated `identifier@16` expressions have the same problem container recovery
  in C and C++. A missing `if` condition recovers across the following complete
  statement and preserves later statements and methods.
- Microsoft assembly in an incomplete `do` drops that control statement,
  recovers across its first following statement, and keeps later siblings.
- An identifier inside an abstract pointer declarator is a syntax problem,
  including Microsoft `__ptr32`/`__ptr64` spellings. Those identifiers remain
  ordinary call names elsewhere.
- The predefined `__extension__` marker disappears before type-id
  disambiguation. A grouped GNU statement-expression remains executable even
  when followed by subtraction or multiplication.

The regression checks require strict extraction, complete function coverage,
exact directed graph isomorphism with entry/exit roles, matching degeneracy,
and the original recovered method end lines for assembly cases. Original
Joern v4.0.150 source commit:
`958fdd3d976197a783f8ade1254b43e977648f28`.
