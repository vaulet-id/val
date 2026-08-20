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

## The printer

`valang::print` writes a program back out in one shape, and `valc --format`
does it in place. It exists for two reasons and the second is the important
one: a file has a form an editor can produce, and the parser has something to
be tested against.

```
print(parse(print(parse(x)))) == print(parse(x))
```

That property, checked over every example in `crates/valang/tests/roundtrip.rs`,
found two disagreements the first time it ran. A lambda's binder on the line
after its brace was silently dropped — the row a list draws from lost its name —
and a function written as an argument rather than after the parentheses was read
as an empty record. Both are shapes the parser claimed to accept and did not.

Two more properties are checked beside it: the capability report of the printed
program equals the original's, so a node the printer dropped shows up as a line
that went missing; and the errors are the same errors, so a formatter cannot
quietly repair a mistake somebody wanted to see.

**It does not carry comments.** The lexer drops them and the AST does not hold
them, so `--format` refuses a file that has any rather than deleting them.
Attaching comments to the nodes they belong to is the work that turns this into
a formatter somebody can run on their own code, and it has not been done.

## What a diagnostic looks like

```
error: a range of 1 to 999999 is more steps than a screen can be made of. The limit is 10000
  --> 14:15
   |
14 |     for (i in 1...999999) {
   |               ^^^^^^^^^^
```

`crates/valang/tests/ui/` holds one program per rule and, beside each, exactly
what the compiler says about it. A test that asks whether a message *contains* a
phrase passes while the rest of the message goes to pieces, and the message is
part of the language. To adopt a change, read it and then run
`VAL_BLESS=1 cargo test -p valang --test ui`.

Reading them the first time is what showed that several diagnostics underlined
the punctuation of the thing rather than the thing: three characters under the
dots of a range, the `.` of a path, the `(` of a call.

## Keeping the two honest

`crates/valang/tests/grammar.rs` holds one test per production the grammar
states something non-obvious about. A production that changes without the file
changing is the disagreement starting again.
