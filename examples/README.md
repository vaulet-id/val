# Examples

These are the fixtures. `crates/valang/tests` compiles them, the playground
opens them, and an example that stops compiling is a change somebody made — which
is the point of keeping them here rather than in a document.

`rejected.val` is the other kind: each of its twenty-one numbered programs must
*not* compile, and carries the error it is owed in a comment. It was written by reading the
specification rather than the compiler, so it is the one place the two are made
to disagree on purpose.

## What each one is for

| | |
| --- | --- |
| [`loyalty.val`](loyalty.val) | the whole action lifecycle — a receipt earns points and reissues a membership |
| [`wallet.val`](wallet.val) | the second file of the loyalty package: a screen, declared data, a press that names an action |
| [`door.val`](door.val) | the small end — prove an age, disclose nothing |
| [`portfolio.val`](portfolio.val) | no state, no issuance, fractional shares without floats, and a proof that discloses nothing |
| [`referendum.val`](referendum.val) | one ballot per voter, counted without a list of who voted. Its handler is Rust |
| [`condo.val`](condo.val) | a vote weighted by a share of a building, with the statutory cap checked on the server. Its handler is Python |
| [`transit.val`](transit.val) | tap to ride, with the fare cap on the operator's server. Its handler is Go |
| [`note.val`](note.val) | a form: the wallet holds what is typed, and the action is handed it |
| [`catalogue.val`](catalogue.val) | every component the wallet ships, on one screen, with what each prop does |
| [`syntax.val`](syntax.val) | the parts of the language that are not about credentials — loops, conditions, defaults, and what to do when something is missing |
| [`kit.val`](kit.val) | components published for another package to draw, and nothing else |
| [`storefront.val`](storefront.val) | imports the kit and declares nothing about how it looks |
| [`rejected.val`](rejected.val) | programs that must not compile, and the error each one is owed |

## Running one

```
cargo run -p valang --features cli --bin valc -- examples/loyalty.val examples/wallet.val
```

A package is several files sharing one scope, so they are given together. The
text bundle beside them is read if there is one, which is why running a single
file out of this directory picks up `text.json` and reports words it should not:
the playground gives each package its own.

`storefront.val` imports `kit.val`, so it needs the package it imports:

```
cargo run -p valang --features cli --bin valc -- --packages <dir> examples/storefront.val
```

where `<dir>` holds one subdirectory per package.
