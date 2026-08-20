# VAL

**Verifiable Application Language** — a declarative language for applications
whose execution can be proved.

An app written in VAL declares what it may do before it does it. Its logic is
pure; its side effects leave the language as *requests* that a host grants or
refuses; and what actually ran can be recorded, replayed and checked afterwards
by somebody who was not there. That is the whole idea. Everything below is a
consequence of it.

**Building a Micro App?** Start with [the guide](docs/guide/01-what-you-are-building.md),
which is written for you rather than for whoever is arguing about the language.
The exact rules are in [`docs/spec.md`](docs/spec.md), and the open questions at
the end of it are still open. What follows is why it is shaped the way it is.

## Why a language rather than a sandbox

A sandbox tells you an app *could not* have done something. It cannot tell you
what the app *did*. Once an application handles credentials, payments or
signatures on somebody's behalf, that is the question being asked — by the
person, by their counterparty, and eventually by a regulator — and no amount of
isolation answers it.

VAL answers it by making effects part of the type system rather than part of
the runtime. A pure phase computes; an effect phase may only request. The host
sees every request before it happens, so consent is a real gate rather than a
dialog, and the record afterwards is a fact rather than a log line an app chose
to write.

> Pure functions calculate. Effects require capabilities.

## Shape

```
input → require → verify → compute → update → execute
```

An action is a function of `(previous state, input, runtime context, code)` to
`(new state, output, effects)`. There is no `Date.now()`, no `random()`, no
`fetch()` in the language: nondeterministic values arrive from the host as part
of the runtime context and are recorded with everything else. That is what
makes replay possible, and replay is what makes the record worth anything.

The language is **total** — no recursion, no unbounded loops, lists consumed by
bounded combinators. Every program halts and the compiler knows it, which is
cheaper than a fuel meter finding out and makes a pure function worth something:
one that might not return is only half a guarantee.

Verification is type narrowing, not an assertion. `verify` is the only way to
obtain a `Verified<T>`, so a function that demands verified data cannot be
handed anything else — the check cannot be forgotten, because forgetting it does
not compile.

## Hosts

VAL does not know what a credential store, a payment rail or a screen is. It
emits `EffectRequest { capability, operation, payload }` and stops. A host
decides whether the capability was declared, whether the person consented,
whether policy allows it, and only then does anything real.

[Vaulet](https://vaulet.id) is the first host and the reason this exists. It is
not the only one it is allowed to have: anything host-shaped is behind an
interface, and a second host is a supported thing rather than a fork. Where the
language needs something Vaulet already has — a canonical encoding for hashing,
for one — the interface comes first and Vaulet's implementation plugs into it.

## What it looks like

```
action ScanToEarn {
  input   { receipt: Credential<PurchaseReceipt> }
  require { state.member exists }

  verify  { const checked = receipt with ReceiptFromMerchant }

  compute {
    const earned = checked.claims.amount / satangPerBaht
    const tier   = tierFor(state.lifetimePoints + earned)
  }

  update {
    lifetimePoints: state.lifetimePoints + earned
    member.tier:    tier
  }

  execute { credential.issue(LoyaltyMember { tier: next.member.tier, … }) }
}
```

`verify` is the only way to obtain a `Verified<ReceiptFromMerchant>` — and the
type names the *policy*, not the credential, so data checked strictly and data
checked loosely cannot be confused for each other. `require` is where an
optional is narrowed and where a defect stops the action; a rule that may fail
in ordinary use belongs in `verify`, where the person is told rather than the
app crashing. `compute` cannot reach an effect and neither can any function,
because there are no effectful functions. `execute` does not issue anything: it
emits a request the host may refuse. Disclosing a claim is an effect and lives
there too.

Longer examples, including one file of programs that must not compile, are in
[`examples/`](examples/).

## Interfaces

An application declares its screens; it does not implement them. The host ships
the components, their behaviour and their state, and VAL says which ones, bound
to what, and which action a press calls — so a press goes through the same
phases, consent and record as everything else, and a screen adds no path to an
effect.

Which tab is open and what is typed but unsubmitted belong to the host, never to
application state: `state` here is hashed, signed and replayable, and it would
be diluted by every press. Props are semantic — no colours, no pixels — and
text comes from the signed manifest, so the compiler can refuse a build with an
untranslated locale, and the host can be the only thing that ever formats a
Thai date.

Applications may use the host's screen archetypes or compose their own from the
same primitives; an archetype *is* a composition the host wrote. Composing
freely is allowed because otherwise every application looks the same. It is safe
because the interface is data and the language is total, so every screen is
rendered at every size, locale and theme at build time, and a layout that breaks
does not ship.

## Layout

| | |
| --- | --- |
| `crates/valang` | front end — parser, typed AST, type checking, capability and trust analysis |
| `crates/valang-runtime` | back end — the tree-walking evaluator, effect requests, execution records |

There is no bytecode VM and there will not be one. v1 walks the typed AST, which
is a few hundred lines and the easiest thing to instrument for an execution
record. Wasm is the destination when there is a reason to go there — untrusted
third-party code needing hard fuel limits and signed bytecode — and it replaces
the back end only.

## Status

Against the pipeline in [§7](docs/spec.md), today:

| stage | |
| --- | --- |
| lexer, parser, AST | **done** |
| semantic analysis | **done** — scopes, names, narrowing, duplicates |
| the text bundle | **done** — checked against the code, in every locale the manifest promises |
| type checking | **done** — `Verified<P>`, provenance, nullability, arity, claim types |
| capability analysis | **done** — name *and* argument, both directions |
| trust analysis | **done** — subject types, anchors, refinement as syntactic containment |
| determinism, totality | **done** — no floats, no clock of its own, acyclic call graph |
| policy validation | **done** — one disclosure per action, patch paths, state changed only by `update`, and no effect reading another's result |
| capability report | **done** — derived from the code |
| IR | not started: both back ends read the typed AST. The Wasm one covers what a pure function is written out of — arithmetic, comparison, `if`, `switch`, ternary, `exists`, `?:`, lists, records, `let` and assignment, destructuring, calls and field access — and refuses the rest, which is a function written in place and the combinators that take one |
| evaluator | **done** — phases, effects as requests, traps, and screens resolved through host capabilities before anything is drawn |
| canonical encoding | **done** — deterministic CBOR, checked against RFC 8949 |
| state Merkle root | **done** — `(path, value)` leaves, one per list element, with inclusion proofs |
| execution record | **done** — code, input, roots, effects, context, outcome, **signed by the host** |
| manifest, text bundle | **done** — a locale missing a key refuses the package |
| integrity, signature, `.va` | **done** — written and read back; a strict decoder refuses any non-canonical encoding; `kind` and the catalogue version go to a host policy the crate does not hold |
| Wasm back end | **done for the pure fragment** — functions compile and run under `wasmi`, with fuel, and the module carries its own constants; actions stay with the host, since `execute` describes effects rather than performing them |

```
cargo run --features cli --bin valc -- examples/loyalty.val
cargo run --bin valrun -- examples/loyalty.val ScanToEarn
```

```
cargo run --bin valpack -- build  <dir> -o app.va
cargo run --bin valpack -- verify <dir>
```

`valc` prints the diagnostics and then the capability report — the whole of what
a host runs over a package it received. `valrun` walks one action and prints the
execution record: roots before and after, the batch the host was offered, and
the state's leaves with their hashes; its wallet is a stub and says so.
`valpack` builds a `.va` and verifies one the way a host would — every source
hashed, the signature over those bytes, the program compiled here rather than
taken on trust, and the shipped report recomputed and compared.

All twelve of `rejected.val`'s programs are refused, each with the message its
own comment says it is owed. The order of work is [§12 of the spec](docs/spec.md).

The tests are the examples, and the examples were written from the specification
before this crate existed: `rejected.val` carries the error each program is owed
in a comment, and the suite asserts the compiler says something with that shape.

Nothing is stable and nothing is published. The crates are `valang` and
`valang-runtime` rather than `val`, because `val` on crates.io is somebody
else's.
