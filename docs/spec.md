# VAL — the language

**Status:** draft, 2026-08-17. Nothing here is built. This is the specification
to argue with; the open questions in §11 are open, and the recommendation under
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

**An application is a package, and a package may be several files.** They share
one scope: there is no per-file namespace and no import statement, because a
file is how the author organises the work, not a boundary anybody else should
have to trace. A reviewer still finds everything with one search.

**There are no imports across packages.** The only things worth sharing are pure
helpers and trust policies — helpers that are genuinely common belong in the
closed set of builtins, and a shared policy is published in a register and named
with its hash, the way the claim vocabulary is (ADR 0046). What is not on offer
is a dependency somebody else can change after you have signed.

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

**`if` and `switch` parenthesise their condition**, as TypeScript and Dart do
and as Rust does not. The reason is not taste: it removes every ambiguity
between a block and a record literal, which is a special case Rust had to add a
rule for.

**Numeric literals** are decimal, with `_` permitted between digits — `100_000`,
never `_1` or `1_`. No hexadecimal, no binary, no exponent: a loyalty scheme has
no use for them and each is a way to write a number a reviewer cannot read at a
glance.

`12.50` **lexes as one token and the parser rejects it**, rather than the lexer
splitting it into `12`, `.`, `50` and producing "expected field name". The whole
of `rejected.val` stands on this: a rule is only taught by the message it
produces.

**Identifiers are ASCII.** Strings are full UTF-8 and Thai text belongs in them,
but a language that decides who somebody is cannot have identifiers where a
Cyrillic `а` and a Latin `a` are different names that look identical. The
readability this gives up is recovered where it belongs: what a person reads is
the consent sheet and the app's own description, which come from the signed
manifest, not from identifiers anybody can choose.

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

**Strings are compared and passed, never built.** No interpolation, no `+`. This
is not about lexer cost: every sentence a person reads must come from something
that was signed, and a string assembled at runtime is a sentence nobody
reviewed — which would make the rule against imitating host chrome unenforceable
the day UI arrives. Text for people lives in the manifest as a template with
named slots, and code supplies the values (§9). Composite keys are structured
arguments, not concatenation: `storage.write(scope: "member", id: memberId, …)`.

**There is no floating point.** Two reasons and either would be enough: NaN bit
patterns are the main source of nondeterminism under Wasm (§8), and money and
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
declared (§11.4).

**The subject is bound explicitly.** `receipt.signature.valid`, never a bare
`signature.valid`: an implicit receiver is unambiguous exactly until a second
credential is in scope.

### Provenance travels with the value

`Verified<P>` covers the credential. It does not cover what is computed from it,
and without that the paradigm stops at the edge: `amount / 100` is an `int` like
any other, and the credential this application signs afterwards says nothing
about where its numbers came from.

So **every value carries the set of trust policies it descends from**, the
compiler propagates it, and nobody writes it by hand except at two boundaries.

```
credential.issue(LoyaltyMember {
  points: next.member.points from { ReceiptFromMerchant }
})
```

The `from` clause is a requirement, not an annotation: this claim may be
computed only from data verified under that policy, and mixing in anything
unverified — or verified under something else — does not compile. The issued
credential then carries the provenance of each claim, machine-checkable by
whoever receives it next. They do not have to take our signature's word for how
the number was reached.

The second boundary is `prove`, where the clause says what is in the witness.

**This stays tractable because the lattice is small.** It is not a hierarchy of
secrecy levels; it is a set of policy names, and the language is total, with no
recursion and no loops, so propagation is set union in a single pass.

**It stays usable because it is inferred everywhere else.** An error points at
the line where an unverified value entered, not at the line where it was finally
used — the latter is what makes information-flow types notorious.

This is the riskiest piece of the type system, and it has an ordered retreat: do
it in the execution record first and in the types later. Strengthening a type
does not break a program that was already correct.

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

- **Termination without fuel.** Wasm's fuel limit (§8) becomes a second belt
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

### State is a Merkle tree, not a blob

The execution record carries a **root**, not a hash of the whole state.

Leaves are `(path, value)` pairs — the same paths `update` patches,
`member.tier` and `lifetimePoints` — each encoded canonically, sorted by path,
built into a binary tree with a defined shape. dCBOR already fixes the encoding
of a value and the ordering of fields, so nothing new has to be invented to make
two implementations agree.

A single hash would have meant that proving anything about state requires
opening all of it. That is the opposite of what the rest of this language does:
a credential can disclose one claim and prove another, while the application's
own state — which is where "gold tier" and "portfolio total" actually live — was
a solid block.

With a root:

```
disclose state.member.tier
prove state.lifetimePoints >= 10_000
```

Each is an inclusion proof against the root that is already in the record, and
Merkle inclusion inside a circuit is a well-worn gadget, so this composes with
§10 rather than needing anything of its own.

**A proof about state is not a proof about a credential, and the difference must
reach the verifier.** A credential claim is backed by an issuer who signed it. A
state field is backed only by the chain of records that produced it — the
application asserted it, correctly, by rules anybody can re-run, but no third
party stood behind the input. Provenance (§4) carries this without new
machinery: a value derived from state has no trust policy in its provenance set,
and a verifier reading a proof over an empty provenance set is being told, in
the type, that this is self-asserted.

Open: **how a list is laid out in the tree.** One leaf per element allows
proving one entry without revealing the others, and reveals the length; one leaf
for the list hides the length and proves nothing selectively.

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

### Nothing here is consensus

This resembles a smart contract and differs in the one way that matters: state
lives on one device and no network agrees about it. The chain of roots is
**tamper-evident, not tamper-proof** — a person can discard the whole chain and
start again, and nothing stops them. What the chain gives is detection, and only
to somebody who kept an earlier record.

That is not a defect to be engineered away; it is the consequence of the state
being the person's own. Two things make it enough in practice, and neither
requires new infrastructure:

- **A verifier remembers the last root it saw** from this person, and refuses a
  record that reaches back behind it. This catches rollback inside a
  relationship, which is where the value is: the loyalty scheme that would be
  defrauded is the one that has been talking to this wallet all along.
- **An issued credential records the root it was derived from.** Rolling back
  then leaves a signed credential pointing at a state that no longer exists in
  the chain, which the issuer can see. The issuer becomes a witness to time
  without being told what the state contained.

A transparency log for the wallet's whole chain would catch the rest. It is a
component to run and a way to leak metadata, and it waits for somebody who needs
it.

### Who signs a credential an application issues

An application has no issuer key and must not have one. `credential.issue`
therefore does not sign anything: it produces an effect request, the device
signs the execution record, and **the publisher's backend verifies that record
and signs the credential with its own key**.

This is why a publisher has a server at all, and it is the whole of what the
server does: it does not run VAL, does not hold state, and does not see it. It
checks the signature, resolves the code hash to a version it published, checks
the trust chains, verifies any proof — the verification key derives from the
compiled circuit, so it knows which predicate was proved rather than being told
— and then either signs or refuses.

Refusing is the point. A person holding the device can rewrite their state; they
cannot make the publisher sign a credential for a run that does not verify.

Where a credential carries no value, the weaker form is available: the device
signs it as "this application, this version, on this device", which is checkable
and self-asserted, and says so.

### Execution record

Application id and version, publisher, code hash, action, input hash, previous
and new state **roots**, policies used, capabilities used, effects requested and
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

An application declares its screens. **It does not implement them.** The host
ships the components, their behaviour and their state; VAL says which ones,
arranged how, bound to what, and which action a press calls.

```
screen Wallet {
  data {
    receipts: credentials of PurchaseReceipt verified with ReceiptFromMerchant
  }

  column {
    card(text: "balance", points: state.member.points)
    tabs {
      tab("history") { list(receipts) { r -> row(text: "receipt", at: r.claims.purchased_at) } }
      tab("rewards") { … }
    }
    button(text: "scan", emphasis: primary, onTap: ScanToEarn)
  }
}
```

### A press calls an action, and nothing else

`onTap` names a declared action. Every press therefore goes through
`input → require → verify → compute → update → execute`, the same consent, the
same execution record. **The interface adds no path to an effect** — there is
nothing a screen can do that an action could not have done, which is what keeps
§5 true once there is a screen at all.

### The host owns interaction state, all of it

Which tab is selected, where the list is scrolled, what is typed in a field that
has not been submitted: none of it is application state. It is not hashed, not
committed, not in the execution record.

The rule that forces this is not tidiness. `state` in this language is hashed,
signed and replayable, and if a scroll position enters it then "provable" is
diluted by every press. An action receives what a form collected **at the moment
it was submitted**, through `input`, which is the mechanism that already exists.

The cost is that an application cannot drive its own interface — it cannot
switch tabs, clear a field, or scroll — except through a declared `navigate`.
That is the same trade as never drawing: control given up in exchange for
behaviour that is correct everywhere without each application getting it right
separately.

### Not everything on a screen is a credential

Prices, news, a catalogue, a transaction history — data that is fetched to be
looked at. Issuing a credential for each of them would be absurd, and the volume
and volatility is exactly why this tier exists.

```
screen Portfolio {
  data {
    holdings: credentials of Holding verified with FromLicensedBroker
    prices:   query broker.quotes(symbols: holdings.symbols) as List<Quote>
  }
}
```

**The application authenticates by presenting a credential, and never touches
the token.** The host performs the presentation, obtains the access token, makes
the request and returns the rows. An application that held a bearer token could
send it somewhere else, and the person consented to it being used, not to it
being had.

What that requires:

- **the audience is in the signed manifest**, never assembled at runtime — which
  strings could not do anyway (§3)
- **obtaining access is a disclosure**, so it declares `disclosure.present`,
  appears in the consent the person gave, and is in the execution record. The
  rows that come back are not the disclosure; the credential handed over to get
  them is.
- consent is once per application and audience — "this app may act as you at
  broker.co.th, showing it your brokerage account credential" — not once per
  screen refresh.

### Three grades of data, and the host draws the difference

| grade | where it came from | provenance |
| --- | --- | --- |
| **issuer-backed** | a claim in a credential | the trust policy it was verified under |
| **self-asserted** | the application's own `state` | empty, but anchored to the chain of roots |
| **origin-asserted** | a query answered by an authenticated API | the audience that answered |

The third is not nothing — the host knows exactly which origin it authenticated
to, so it can say "from broker.co.th, unsigned" rather than "unverified". But it
is not a signature, and the person must be able to see which numbers on a screen
somebody stood behind.

**The host renders the difference, and the application cannot choose how.** Same
rule as consent chrome: an application that could make fetched figures look
issuer-backed would break the one promise the wallet makes.

### Fetched data and the record

Query results are **not** recorded — an execution record full of news headlines
buries what it exists to prove.

But the moment such a value crosses into `compute`, `update`, `issue` or
`prove`, the state depends on something that cannot be replayed, and the chain
is worth nothing. So: **a value that crosses that line is recorded in the
runtime context**, the way an oracle input is, and replay works again.

The compiler can tell which is which, because provenance already tracks it —
and `from { … }` on an issued claim (§4) rejects the crossing outright where the
claim declared a policy no query can satisfy.

### Presets and free composition are one system

The host ships **archetypes** — a list screen, a detail screen, a form screen, a
tabbed screen — and an application may use one, or compose its own from the same
primitives.

They are not two mechanisms. **An archetype is a composition the host wrote**,
in the same data, rendered by the same renderer, versioned the same way. One
semantics, and the host is using the thing it ships.

Composing freely is allowed because otherwise every application looks identical
and the catalogue's authors become the bottleneck on everybody else's product.

### Freedom in composition, not in geometry

- **Semantic props only** — `text:` (a manifest key), `icon:` (from the
  catalogue), `emphasis:`, `state:`, `onTap:`. No colours, no fonts, no pixel
  sizes. Spacing comes from a scale.
- **No absolute positioning.** A container owns its own overflow.
- **Text is measured and wrapped by the host**, because it came from the
  manifest and because Thai runs about a third longer than English.

Free composition is where a design system usually starts breaking on small
screens, long text and dark mode. The answer is not to forbid it:

### Layout is checked at build time

The interface is data and the language is total, so tooling renders every screen
at every size, locale and theme **before it ships**, and a build fails on
anything that overflows or falls below contrast.

An application that composes its own screens and looks unlike the others is
working as intended. One that composes its own screens and breaks on a small
phone in Thai does not get published.

### Text comes from the manifest, checked by the compiler

```
"balance": { th: "คุณมี {points} แต้ม", en: "You have {points} points" }

card(text: "balance", points: state.member.points)
```

The compiler reads the manifest — they are signed as one package, so checking
them apart would mean signing a pairing nobody verified — and rejects a missing
key, a missing slot, a slot of the wrong type, **and a locale that is not
translated.** A market's language missing is a build failure, not a bug report.

Numbers, dates and currency are formatted by the host per locale. An application
cannot get Thai numerals or Buddhist-era dates wrong, because it never touches
them.

### What holds regardless

- **Consent is host chrome.** No application draws it, covers it, or imitates
  it.
- **Branding comes from the signed manifest**, never from styling.
- **UI is data, not code** — signed, diffed and audited with the same canonical
  encoding as the execution record.
- **Never arbitrary drawing.** A publisher who needs pixels wants the webview
  tier, and pays for it with the capabilities that tier cannot have.

---

## 10. Proofs

`prove` compiles to a circuit. That is only possible for a fragment of the
language, and **the compiler must know which fragment**, because the rule in §5
— that a proof may never quietly degrade into a disclosure — cannot be kept by a
compiler that finds out at proving time.

So there is a **provable subset**, checked statically, and leaving it is an
error with a message that says which line and why.

Inside it:

- integers with a declared width — `int<32>` — because range checks are the
  dominant cost and they scale with bits
- `date` and `datetime` as integers, compared
- string equality; no construction, which §3 already forbids everywhere
- `switch` and `?:`, with the cost of **every** branch, not the taken one — this
  has to be stated or nobody will understand why a proof is slow
- list combinators where the length is known at compile time
- **Merkle inclusion against the state root**, so a proof may be about state as
  well as about claims — with the provenance distinction of §7 reaching the
  verifier rather than being flattened into "verified"

Outside it: effects, and anything whose size is not statically known.

What the host supplies, and what VAL does not: the circuit proving that the
claims came from a credential an anchor-resolvable issuer signed. VAL provides
the predicate over those claims, and the two compose over the witness.
`disclose` marks a public input; everything else the proof touches is witness.
Proving randomness never enters the execution record — the statement and the
verdict do.

---

## 11. Open questions

1. **What exactly must a host provide?** The interface is named throughout this
   document and specified nowhere. Writing it down is the honest test of whether
   the first host has leaked into the language, and it blocks a second host. It
   now covers three things, not one: capabilities, the text bundle, and the
   component catalogue.
2. **How is the component catalogue versioned?** An application signed against
   catalogue v1 will run on a host shipping v3. *Recommended:* the package
   records the catalogue version it was built against and the host renders those
   semantics or refuses — the interface is data, so keeping old semantics is
   possible in a way that keeping old code is not.
3. **How does navigation work?** `navigate` is named and unspecified. It decides
   whether an application has a screen stack, and whether it can send somebody
   anywhere they did not ask to go.
4. **May one policy be declared to refine another?** `trust A refines B { … }`
   would let a function accept `Verified<A>` where `Verified<B>` is demanded.
   *Recommended:* yes, eventually, and syntactic containment only — never
   semantic implication.
5. **Is there a standard library, and how small?** *Recommended:* a fixed, closed
   set of host-implemented builtins — durations, list combinators, string
   comparison — with no way for an application to add to it. A DSL with an
   extensible prelude is a general-purpose language that has not admitted it.
   Totality is the reason it must be closed: a builtin is the one place a
   non-terminating operation could enter.
6. **`fold` and totality.** A fold whose accumulator grows is bounded in steps
   but not in memory. *Recommended:* bound value sizes at the host interface,
   and say so there rather than pretending totality covers it.
7. **How is a list laid out in the state tree?** One leaf per element proves a
   single entry and reveals the length; one leaf for the whole list hides the
   length and proves nothing selectively. *Recommended:* per element, with
   padding to a declared bound where the length is itself sensitive.
8. **`type` is declared in §2 and specified nowhere.** Plain records exist, and
   whether they may hold verified and unverified data side by side is a
   provenance question (§4), not a syntax one.

---

## 12. Order of work

1. Parser and typed AST for the shell plus expressions.
2. Type checker: `Verified<P>`, nullability, exhaustive switching, trapping
   arithmetic, effect placement, acyclic call graph.
3. Capability and trust analysis over the typed AST.
4. Tree-walking evaluator, effect requests, execution records — provenance in
   the record first, in the types after.
5. The host interface (§11.1), and one capability wired end to end.
6. Screens: the archetypes that exist as host UI already, then composition, then
   the build-time layout check.
7. Everything else — Wasm back end, the provable subset, packaging — after a
   real application exists and pushes on it.
