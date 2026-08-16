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

### Two readers

A VAL program has two audiences and they are not the same people.

**The shell is read by people who do not write code.** A partner deciding
whether to publish an app, a compliance officer asking what it touches, a
security reviewer, the person whose credentials it wants. They read
`capabilities`, `trust`, `require`, `verify` and `execute`, and they must be
able to answer "what can this do to me" without being taught a language first.

**The expression layer is written by people who do.** Arithmetic, string
handling, a tier lookup. It should be boring and familiar, which is what
borrowing from TypeScript and Dart buys.

**Where the two conflict, move the work into the shell.** That is the rule, and
it has already changed three things: `update` is a table of fields rather than
nested spreads, `require` says `exists` rather than `!= null`, and a lookup that
would be a chain of `?:` is a `switch` that reads as a table. Each moved a
decision out of the half nobody outside can read.

The cost is a larger shell — more keywords, more blocks, more grammar. That is
the right place to spend, because the shell is the part that gets read a hundred
times more often than it gets written.

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

### Grammar rules that everything else assumes

**A newline ends a statement, and there are no semicolons** — except inside an
unclosed bracket, where the statement continues. A long expression is wrapped in
`( … )` on purpose, which makes complexity visible instead of implicit. The rule
that a line ending in an operator continues is deliberately not adopted: a
language selling the absence of surprises should not import the one rule an
entire industry has been guessing wrong about for twenty years.

**Separators follow the two readers.** The shell is newline-separated; inside an
expression, elements are comma-separated. `member.tier: tier` on its own line,
`{ ...member, tier: tier }` with commas. One rule, and it is the same rule that
decided everything else about who reads what.

**Keywords are reserved, not contextual.** The check is not theoretical: of the
57 claims in the reserved vocabulary today, none collides with a keyword — and
the vocabulary avoids `state` by spelling it `address.region`, as OpenID Connect
does. Contextual keywords cost a worse error message on every mistake, and they
are being bought here against a collision that does not exist.

**A dot is always field access.** Claim names in the register are written as
paths — `address.postal_code`, `address.region` — because a credential holds
those apart (ADR 0031), and that is structure rather than a name with a dot in
it. The credential's type turns it into structure and VAL reads it as structure.
The consequence must be stated rather than discovered: **`claims.address` on its
own is not a value**, because no issuer ever signed a claim by that name, and
the compiler says so instead of handing back a record with half its fields
empty.

**Names this program chooses are camelCase** — `lifetimePoints`, `tierFor`.
**Claim names it does not choose are left alone**: `purchased_at`,
`document_number` and the rest come from the issuer's vocabulary, and rewriting
them to look like local code would hide the one fact that matters about them,
which is that somebody else defined them.

**Arguments are named once there are two of them.**

```
payment.request(to: merchant, amount: 12000)     // reads without documentation
payment.request(merchant, 12000)                 // rejected
```

A single argument may be positional — `tierFor(total)` — because there is
nothing to confuse it with. Beyond one, the call site is read far more often
than it is written, frequently by somebody deciding whether to approve what it
does, and an unlabelled pair of numbers is exactly where that reader stops.

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

A `state` field declares its starting value with `default`, not `=`:

```
state {
  member:         LoyaltyMember?
  lifetimePoints: int default 0
}
```

`=` appears nowhere in this language, and one exception in the declaration of
persistent state is the worst place to keep it.

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
field or drop one.

This is how a record is derived **in an expression**. State is not derived this
way: `update` is a patch table (§5), because nested spreads are the shape of
this language that a non-programmer stops being able to follow first.

Either way there is no `a.b.c = v` anywhere in the language, in any phase.

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

### Every credential has the same four faces

```
receipt.claims        // what the issuer said — the vocabulary, typed
receipt.signature     // .valid
receipt.status        // .active, and why it is not
receipt.holder        // .bound — is this the person in front of us
```

`claims` is the credential's own type; the other three are the same on every
credential in the language, because they are what a trust policy is written
about. They are readable in `trust` and in `verify`, and nowhere else: an
application deciding for itself whether a signature is good enough is the thing
`trust` exists to stop.

### A policy is part of the type

```
trust ReceiptFromMerchant(receipt: PurchaseReceipt) {
  anchor: "th.co.codefin.merchants"
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

Names declared in `input` are in scope for every later phase, bare — `receipt`,
not `input.receipt`, the way a function's parameters are. The prefixed roots are
the ones that come from somewhere else: `state.`, `context.`, and `next.` in
`execute`.

Narrowing lives in `require`, and it is spelled in words:

```
require {
  state.member exists
  amount > 0
}
```

`exists` rather than `!= null`, because the people who most need to read this
block are the ones for whom `null` is jargon. It is a shell keyword, and the
shell is allowed to invent.

### `compute` is pure, and so is every function

```
function tierFor(points: int): Tier {
  return switch (points) {
    >= 10000 => Tier.gold,
    >= 2000  => Tier.silver,
    default  => Tier.bronze,
  }
}
```

A `switch` arm may be a comparison, so a lookup reads as a table rather than as
a chain of `?:`. The conditional operator stays for the two-way case, where a
table would be heavier than what it replaces.

**Arms are tried in order, and an arm that can never be reached is an error.**

```
switch (points) {
  >= 2000  => Tier.silver,
  >= 10000 => Tier.gold,      // error: unreachable
  default  => Tier.bronze,
}
```

A language that refuses `default` over an enum for safety cannot then let a dead
arm through in silence. Order-dependence is fine; order-dependence nobody can
see is not.

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
  lifetimePoints: total
  member.points:  state.member.points + earned
  member.tier:    tier
}
```

**`update` is a patch, not an expression.** Each line names a field of the state
and the value it takes; everything not named is unchanged. It reads as a table
of what this action changes, which is exactly the question somebody reviewing
the app is asking.

`:` and not `=`, because nothing is being assigned: the block describes the next
state, and the previous one is still readable as `state.…` on the right of every
line. A path may name nested fields, and may not contain a list index — that is
where a patch would need an optics story, and the answer is to compute the new
list in `compute` and name it here in one line.

**Only paths, never a record literal.** `member: { ...state.member, tier: tier }`
says the same thing as `member.tier: tier` and is the second way to do one thing,
which is what this language spends its budget avoiding. Build the record in
`compute` and name it here if it is genuinely a whole new record.

The block produces the next state and binds it as **`next`**, which `execute`
reads. Without that binding an application recomputes the same arithmetic in
both phases, they drift the day somebody edits one of them, and the execution
record proves the disagreement faithfully.

The host commits the next state **only if the effects in `execute` succeed** —
the alternative is a state that records something that did not happen. What that
sentence requires of `execute` is below.

### `execute` is where effects appear, disclosure included

```
execute {
  credential.issue(LoyaltyMember { points: next.member.points, … })

  present {
    disclose checked.claims.country
    prove checked.claims.birthdate <= context.time.now - duration(years: 20)
  }
}
```

**Disclosure is an effect.** It hands a person's data to somebody else, which in
a system built around privacy is the most consequential thing an application can
do. It therefore requires a declared capability, appears in `execute` with the
rest, and lands in the execution record as an effect — not as a footnote.

### `execute` requests a set, not a sequence

The effects in `execute` are **one batch, offered together**. The host approves
the whole batch or refuses the whole batch, and the state from `update` is
committed only if the batch ran. No effect may read the result of another: if
one genuinely depends on another's outcome, that is two actions, and the person
gets to see both.

This is not a convenience. Without it, "the state commits only if the effects
succeed" is a sentence that cannot be kept: issue a credential, then fail to
disclose, and the credential is already out.

**Some effects cannot be taken back at all.** A disclosure is the clearest —
there is no operation that un-tells somebody a postcode. So:

- **an action performs at most one disclosure.** Two disclosures in one batch
  cannot both be conditional on the whole batch succeeding, because the first
  has already happened by the time the second is refused.
- **irreversible effects run last in the batch**, after everything the host can
  still walk back. The compiler orders them; the author does not have to know.

An application that wants to disclose twice wants two consents, which is what
two actions give it.

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

### State outlives the code that wrote it

`version` is on the first line of every program, and a published application
will be replaced by a later version while somebody's `state` sits on a device
written by the earlier one.

**A change to the shape of `state` is a new version, and its state starts
empty.** No migration, no compatibility shim, no dual reader. This is the same
rule that already governs an app's kind and its capabilities: the person
consented to a description, the description changed, so they are asked again —
and what they are asked is legible precisely because there is no third state
that is half of each.

The alternative is worse than it looks. A migration is code that runs against
data the current version never produced, is exercised by nobody, and cannot be
replayed from an execution record, because no action performed it. In a language
whose entire claim is that what ran can be proved, an unprovable step at the
boundary between versions is not a small exception.

An application that cannot afford to lose state should keep it where state
belongs — in a credential it issued, or on its own backend — rather than in a
field it can silently rename.

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
