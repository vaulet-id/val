# VAL

**Verifiable Application Language** — a declarative language for applications
whose execution can be proved.

An app written in VAL declares what it may do before it does it. Its logic is
pure; its side effects leave the language as *requests* that a host grants or
refuses; and what actually ran can be recorded, replayed and checked afterwards
by somebody who was not there. That is the whole idea. Everything below is a
consequence of it.

**Nothing here works yet.** The specification is [`docs/spec.md`](docs/spec.md)
and the open questions at the end of it are still open. What follows is why it
is shaped the way it is.

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
  require { state.member != null }
  verify  { receipt with ReceiptFromMerchant }

  compute {
    const earned = receipt.claims.amount / 100
    const tier   = if earned > 10000 { Tier.gold } else { Tier.silver }
  }

  update  { member.points = state.member.points + earned }
  execute { credential.issue(LoyaltyMember { tier: tier, … }) }
}
```

`verify` is the only way to obtain a `Verified<PurchaseReceipt>`, `require` is
where an optional is narrowed, `compute` cannot reach an effect, and `execute`
does not issue anything — it emits a request the host may refuse. Longer
examples, including one file of programs that must not compile, are in
[`examples/`](examples/).

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

A specification and a skeleton that compiles and does nothing. The order of work
is [§8 of the spec](docs/spec.md#8-order-of-work). Nothing is stable and nothing is
published. The crates are `valang` and `valang-runtime` rather than `val`,
because `val` on crates.io is somebody else's.
