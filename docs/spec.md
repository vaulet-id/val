# VAL — the language

**Status:** draft, 2026-08-17. Nothing here is built. This is the specification
to argue with; the open questions in §9 are open, and the recommendation under
each is what the rest of the document assumes until somebody says otherwise.

VAL is a **domain-specific language, not a general-purpose one**, and it is
**total**: every program halts, and the compiler knows it (§6). The design
principle is a declarative shell with a small expression layer — the outer
structure is data, the inner expressions are familiar to anyone who has written
TypeScript or Dart.

That second half is a constraint, not a mood. The shell may invent whatever it
needs, because nothing else looks like it. The expression layer may not: it
borrows `const`, `function`, `switch`, `?:`, spread, `T?` and `List<T>` from
languages the reader already has, and where a construct would have to be
invented, the answer is usually that the shell should have handled it. A DSL
whose expressions are a fourth dialect costs its reader twice.

---

## 1. What VAL is for

An application written in VAL declares what it may do before it does it, and
what it actually did can be checked afterwards by somebody who was not there.

That is the whole of it. The properties usually listed — declarative structure,
immutability, determinism, capability-based security, explicit effects — are not
a feature list. Each is there because dropping it would break the sentence above.

**A sandbox and this are not the same thing.** A sandbox establishes that an
application *could not* have done something. It says nothing about what the
application *did*. Once an application acts on somebody's behalf — presenting a
credential, moving money, signing a payload — that second question is the one
being asked, and isolation does not answer it.

### The host

VAL has no I/O. It does not know what a credential store, a payment rail, a
network or a screen is. It computes, and where an effect is called for it emits
a description of one:

```
EffectRequest { capability, operation, payload }
```

A **host** turns those into reality, or refuses them. It decides, in order: is
the capability declared · did the person consent · does host policy allow it ·
is the application trusted · is the operation in scope. Only then does anything
happen.

[Vaulet](https://vaulet.id) is the first host and the reason VAL exists. It is
not privileged in the language. Where the language needs something only a host
can supply — a canonical encoding, a clock, randomness, trust resolution — the
interface is specified here and the host implements it.

---

## 2. Shape of a program

```
app "th.co.codefin.loyalty"
version 1

capabilities { … }        // everything this app may ever ask for
enum · credential · type  // declarations
state { … }               // what persists between actions
trust … { … }             // named verification policies
function … { … }          // pure helpers
action … { … }            // the only executable thing
```

The application identifier is a **quoted string**, not a bare dotted name: a
reverse-DNS identifier and a field access are the same shape, and a lexer should
not have to tell them apart by context.

Statements are newline-separated. There are no semicolons.

---

## 3. Values and types

`string int bool date datetime bytes`, custom record types, enums,
`List<T>`, `T?`, `Verified<P>`, `Proof<bool>`.

**`int` is 64-bit and signed, and arithmetic traps on overflow.** Trapping is
the only deterministic option — wrapping produces a wrong answer that the
execution record would then faithfully prove, which is worse than failing. A
trap aborts the action and commits nothing.

**There is no floating point.** Two reasons and either would be enough: NaN bit
patterns are the main source of nondeterminism under Wasm (§7), and money and
points want integers or fixed point regardless. Amounts are minor units.

Local bindings use `const`. There is no `var` and no assignment: every binding
is final, and a record is derived rather than changed.

`const` and not `let`, even though nothing here is mutable and the distinction
`const` usually marks does not exist. The word is chosen for the reader: to
anyone arriving from TypeScript, `let` announces a variable that will be
reassigned, which is the opposite of what every binding in this language is.

### Records are derived, never mutated

```
const bumped = { ...member, points: member.points + earned }
```

Spread produces a new value with the named fields replaced. It is typed: the
result is the same record type as the value spread, so spreading cannot invent a
field or drop one. It is the only way to derive a record from another — there is
no `a.b.c = v` anywhere in the language, in any phase, because a dotted
assignment reads as mutation and needs an optics story the moment a list appears
in the path.

### Enums are switched exhaustively

```
const discount = switch (tier) {
  Tier.bronze => 0,
  Tier.silver => 5,
  Tier.gold   => 10,
}
```

**A `switch` over an enum may not use a default arm.** Adding `Tier.platinum`
must break every program that decides something per tier — that is the entire
value of having enums rather than strings. Switches over open domains (`int`,
`string`) require a `default` instead, because they cannot be exhaustive.

### Conditionals

`if` is a statement. The expression form is the conditional operator, as in
TypeScript and Dart:

```
const fee = amount > 100_000 ? 0 : 20
```

A block-bodied `if` that evaluates to a value is Rust's shape, not this
language's, and mixing the two gives two ways to write one thing.

---

## 4. Verification

### `verify` is the only way to obtain a `Verified<P>`

There is no cast, no constructor, no runtime assertion that leaves the type
unchanged. A function that demands verified data cannot be handed anything else,
so the check cannot be forgotten: forgetting it does not compile.

### A policy is part of the type

```
trust ReceiptFromMerchant(receipt: PurchaseReceipt) {
  anchor { "th.co.codefin.merchants" }
  require {
    receipt.signature.valid
    receipt.status.active
    receipt.holder.bound
    receipt.claims.amount > 0
  }
}
```

The result of verifying against that policy has type
**`Verified<ReceiptFromMerchant>`** — the policy, not the credential type.

This is the correction that matters most in the language. `Verified<Employee>`
would be nearly worthless: data checked against a strict policy and data checked
against `{ signature.valid }` would share a type, so a function demanding
`Verified<Employee>` would silently accept the weaker one, and the guarantee
would be decorative. Naming the policy in the type is what makes it real.

The credential type is not named alongside it because the policy already
determines it. Two parameters that cannot disagree are two parameters that will
eventually be written as if they could.

**Policies are nominal.** `Verified<A>` is not `Verified<B>` even if A's
predicates imply B's. Deciding implication between arbitrary predicates is not
something a compiler should attempt, and a relationship that matters can be
declared (§9.2).

**The subject is bound explicitly.** `receipt.signature.valid`, never a bare
`signature.valid`: an implicit receiver is unambiguous exactly until a second
credential is in scope.

### Provenance

A derived value should know what it came from:

```
adult ← age ← AgeCredential ← ReceiptFromMerchant ← { anchor, signature,
                                                     status, binding }
```

---

## 5. Actions

An action is the only executable thing, and it is a function:

```
(previous state, input, runtime context, code) → (new state, output, effects)
```

Replay, audit and state hashing all follow from that signature. Nothing else in
this document is worth much without it.

```
input → require → verify → compute → update → execute
```

Phases may be omitted but not reordered.

| phase | |
| --- | --- |
| **`input`** | what the action is given |
| **`require`** | preconditions and narrowing |
| **`verify`** | trust policies |
| **`compute`** | pure calculation |
| **`update`** | the next state |
| **`execute`** | effects — the only phase where any appear |

### `require` and `verify` fail differently, and that is why there are two

Both refuse to continue and both narrow types. Splitting them on how they *read*
would be a matter of taste; splitting them on how they *fail* is a real
distinction that the host and the person both need:

- **`require` failing is a defect.** The application asked to spend points it had
  already established it might not have. Nobody should see a message about it;
  the action aborts, nothing commits, and it belongs in a bug report.
- **`verify` failing is an ordinary outcome.** The receipt was forged, expired,
  or issued by somebody outside the anchor. The person is told, plainly, and the
  application has not done anything wrong.

An application that puts a business rule in `require` gets a crash where it
wanted a message, and the error should say so.

Narrowing lives in `require`: an action touching `state.member: MemberCard?`
must require it non-null before anything else may read through it.

### `compute` is pure, and so is every function

```
function tierFor(points: int): Tier {
  return points >= 10000 ? Tier.gold
       : points >= 2000  ? Tier.silver
       : Tier.bronze
}
```

**Functions are always pure. There is no effect polymorphism, because there are
no effectful functions to be polymorphic over.** Effects are syntax, permitted
only inside `execute`, and they cannot hide behind a call.

The cost is real and is accepted: a sequence of effects used by three actions
cannot be factored into a helper, and must be written out three times. In
exchange, "what can this action do" is answerable by reading one block, by a
person and by a tool, without following a call graph. For a language whose
purpose is that question, the trade is not close.

### `update` describes the next state

```
update {
  {
    ...state,
    lifetime_points: total,
    member: { ...state.member, points: state.member.points + earned },
  }
}
```

The block evaluates to the new state. The host commits it **only if the effects
in `execute` succeed** — the alternative is a state that records something that
did not happen.

### `execute` is where effects appear, disclosure included

```
execute {
  credential.issue(LoyaltyMember { … })

  present {
    disclose receipt.claims.country
    prove receipt.claims.birthdate <= context.time.now - duration(years: 20)
  }
}
```

**Disclosure is an effect.** It hands a person's data to somebody else, which in
a system built around privacy is the most consequential thing an application can
do. It therefore requires a declared capability, appears in `execute` with the
rest, and lands in the execution record as an effect — not as a footnote.

`disclose` hands over a value; `prove` hands over an answer. `prove` produces a
`Proof<bool>` and nothing weaker: **where the host cannot produce a real
zero-knowledge proof, the compiler must refuse to build the application** rather
than fall back to disclosing the birthdate and comparing it. The author wrote
`prove` and would never learn it had not happened.

### Capabilities name types, not strings

```
capabilities {
  credential.read(PurchaseReceipt)
  credential.issue(LoyaltyMember)
  disclosure.present
}
```

The credential type is declared in the same program. A string here would be an
unchecked second copy of a name, and the first typo would be found by a customer.

---

## 6. Totality

**Every VAL program halts.** There is no recursion — the call graph must be
acyclic, and the compiler checks it — and there are no unbounded loops. `List<T>`
is consumed by bounded combinators only:

```
map · filter · fold · any · all · count · first
```

each of which visits a list that already exists and cannot extend it. Input list
lengths are bounded by the host.

This is a deliberate position, not a missing feature. What it buys:

- **Termination without fuel.** Wasm's fuel limit (§7) becomes a second belt
  rather than the only one.
- **A cost bound before running.** Tooling can price an action, and a preview
  cannot hang.
- **Purity that is worth something.** A pure function that might not return is
  only half a guarantee.

What it costs: the day somebody genuinely needs unbounded iteration, the answer
is that they are writing a service, not a VAL application. That answer needs to
stay true, which means watching what people reach for.

---

## 7. Execution and its record

```
VAL source → parser → AST → semantic analysis → type check →
capability analysis → trust analysis → totality check → IR →
(evaluator | Wasm) → host runtime → platform
```

### Determinism is a language property, not a runtime one

There is no `Date.now()`, no `random()`, no `fetch()` and no filesystem **in the
language**. Nondeterministic values arrive from the host in an explicit runtime
context — `context.time.now`, `context.random.uuid` — and are recorded with
everything else.

### Canonical encoding

State, input and code hashes need one canonical encoding, and it must be the
same one everywhere: a second canonicalisation is a second thing to get subtly
wrong. Deterministic CBOR (dCBOR) is the intended shape. A host that already has
one supplies it through the interface rather than the language carrying a rival.

### Package

Manifest, code, types, credentials, capabilities, assets, runtime version,
integrity, signature — signed. It answers who published this, which version is
running, whether it was modified, what it may do, and what is executing.

### Execution record

Application id and version, publisher, code hash, action, input hash, previous
and new state hashes, policies used, capabilities used, effects requested and
effects executed — **disclosures among them** — runtime context, timestamp,
signature.

---

## 8. Compilation target

**No bespoke VM, now or later.** Everything a hand-built VM would provide —
sandboxing, resource limits, determinism, portability — Wasm already provides,
and maintaining one is a multi-year commitment to the part of the system that is
not the point.

**v1 does not compile at all.** It walks the typed AST in Rust: a few hundred
lines, deterministic, and by far the easiest thing to instrument for an
execution record.

**Wasm is the destination, reached when there is a reason** — untrusted
third-party code needing hard fuel limits and signed bytecode. The front end
does not change; only the back end.

- Wasm core has only `i32`/`i64`/`f32`/`f64`. Avoid the allocator problem by
  keeping values **host-side and passing `i32` handles**, with imported helpers
  (`val_field`, `val_add`, …). The compiler then emits calls and control flow
  and nothing else, which `wasm-encoder` makes mechanical.
- The no-floats rule in §3 exists mostly for this.
- Trapping integer overflow maps to Wasm traps directly.
- **iOS forbids JIT**, so an interpreting runtime is required. `wasmi` or `wasm3`
  inside a Rust core covers iOS, Android and desktop from one integration;
  browsers come free. Check the state of those runtimes and of WasmGC before
  committing — this moves faster than a specification does.

---

## 9. User interface

**v1 has no `screen`.** An application is actions, trust policies and state; the
host draws whatever it draws. This is not a placeholder: a language that renders
is a much larger language, and no application has yet failed to be expressible
without it.

When `screen` arrives, in ascending order of cost: a **host component catalogue**
with typed props · then **constrained layout primitives** · **never arbitrary
drawing.**

Constraints that hold regardless:

- **Consent is host chrome.** The application cannot draw it, cover it, or
  imitate it.
- **Branding comes from the signed manifest**, not from free styling.
- **UI is data, not code**, signed and diffed with the same canonical encoding
  as the execution record.
- Host drawing earns accessibility, internationalisation and dark mode once.

---

## 10. Open questions

1. **What exactly must a host provide?** The interface is named throughout this
   document and specified nowhere. Writing it down is the honest test of whether
   the first host has leaked into the language, and it blocks a second host.
2. **May one policy be declared to refine another?** `trust A refines B { … }`
   would let a function accept `Verified<A>` where `Verified<B>` is demanded,
   with the compiler checking that A's predicates include B's syntactically.
   *Recommended:* yes, eventually, and syntactic containment only — never
   semantic implication.
3. **Is there a standard library, and how small?** `duration(days: 30)` is
   already needed by two example programs. *Recommended:* a fixed, closed set of
   host-implemented builtins — durations, list combinators, string comparison —
   with no way for an application to add to it. A DSL with an extensible prelude
   is a general-purpose language that has not admitted it.
4. **Are reads effects?** A credential lookup touches host state, can fail and
   may prompt — but it is not a mutation, and forcing it into `execute` makes
   ordinary code awkward. *Recommended:* an action declares its data
   dependencies in a phase of its own rather than calling mid-computation.
5. **`fold` and totality.** A fold whose accumulator grows is bounded in steps
   but not in memory. *Recommended:* bound value sizes at the host interface,
   and say so there rather than pretending totality covers it.

---

## 11. Order of work

1. Parser and typed AST for the shell plus expressions.
2. Type checker: `Verified<P>`, nullability, exhaustive switching, trapping
   arithmetic, effect placement, acyclic call graph.
3. Capability and trust analysis over the typed AST.
4. Tree-walking evaluator, effect requests, execution records.
5. The host interface (§10.1), and one capability wired end to end.
6. Everything else — Wasm back end, `screen`, packaging, proofs — after a real
   application exists and pushes on it.
