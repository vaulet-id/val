# The grammar

`grammar.ebnf` is VAL as the parser reads it, written from
`crates/valang/src/parse.rs` and `lex.rs` production by production rather than
from the prose. The two exist to be compared: the specification says what the
language means, the grammar says what it accepts, and where they disagree one
of them is a bug.

## What is not in the notation

Two rules the parser enforces that EBNF cannot state:

**A newline ends a statement, and the lexer decides which newlines exist.** It
emits one only at bracket depth zero, counting `(` and `[`. A brace holds
statements, so a newline inside one is a separator rather than whitespace —
which is why a `switch` arm ends at its line and its comma is optional.

**A `${…}` inside a backtick string is parsed by this same grammar**, and a
brace inside a string within it does not close the interpolation.

## What the comparison found

Writing it down was worth doing before anything else: three of the four
productions that were awkward to state turned out to be places where the parser
accepted something the language did not mean.

| | |
| --- | --- |
| `anchor: shop.example.com` | Read as a single token, so the anchor became `shop`. A policy trusted a root nobody wrote. Quoted now, as every other external name in this language is. |
| `enum Tier { bronze silver }` | Accepted, with no separator. Two ways to write one thing, and one of them is a comma somebody forgot. A comma or a line is required. |
| `let { a, b } = row` | Refused, while `const { a, b } = row` worked. Two binding forms and only one of them took a record apart. |

None of them was reachable from anything in `examples/` or in the guide, which
is exactly why they had survived: the corpus was written by somebody who already
knew the intended shape.

## Keeping the two honest

`crates/valang/tests/grammar.rs` holds one test per production the grammar
states something non-obvious about. A production that changes without the file
changing is the disagreement starting again.
