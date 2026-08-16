# VAL — the language

**Status:** draft, 2026-08-17. Nothing here is built. This is the specification
to argue with; the open questions in §7 are open, and the recommendation under
each is what the rest of the document assumes until somebody says otherwise.

VAL is a **domain-specific language, not a general-purpose one**. The design
principle is a *declarative shell with a TypeScript/Dart-like expression layer*:
the outer structure is data, the inner expressions are familiar.

---

## 1. What VAL is for

An application written in VAL declares what it may do before it does it, and
what it actually did can be checked afterwards by somebody who was not there.

That is the whole of it. The properties usually listed — declarative structure,
functional expressions, immutable by default, deterministic execution,
capability-based security, explicit side effects — are not a feature list. Each
one is there because dropping it would break the sentence above.

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

A **host** is whatever turns those into reality. The host decides, in order:
is the capability declared · did the person consent · does host policy allow it
· is the application trusted · is the operation in scope. Only then does
anything happen. This is the security model, and the language is what makes it
checkable ahead of time rather than observable after the fact.

[Vaulet](https://vaulet.id) is the first host and the reason VAL exists. It is
not privileged in the language. Where the language needs a capability a host
must supply — a canonical encoding, a clock, a source of randomness — the
interface is specified here and the host implements it.

---

## 2. Language surface

Declarative shell:

```
app · version · capabilities · credential · enum · state
action
input · require · verify · compute · update · execute
trust · present
```

Expression layer: `const`, `if` as statement **and** expression, calls, field
access, `T?`, `List<T>`, `"${…}"`. No `var` — local values are immutable.

Types: `string int number bool date datetime bytes`, custom types, generics
(`Credential<T>`, `List<T>`, `Result<T, E>`), Dart-style optionals (`T?`).

**No floating point.** Two reasons and either would be enough: NaN bit patterns
are the main source of nondeterminism under Wasm (§5), and money and points want
integers or fixed point regardless.

First-class verifiability types: `Credential<T>` · `Verified<T>` · `Proof<T>`.

### Action lifecycle

```
input → require → verify → compute → update → execute
```

An action need not use every phase, but it may not reorder them.

| phase | |
| --- | --- |
| **`input`** | what the action is given |
| **`require`** | capability requirements and preconditions — and where narrowing happens: an action touching `state.member: MemberCard?` must require it non-null before `update` |
| **`verify`** | trust rules; multiple expressions are implicitly ANDed. Reads as policy rather than as procedure, which is the point |
| **`compute`** | pure. No effects. An effect call here is a compile error |
| **`update`** | declarative state transition — conceptually an immutable spread, not mutation |
| **`execute`** | the only place effects appear |

> **Pure functions calculate. Effects require capabilities.**

### Trust policies

```
trust ReceiptFromMerchant {
  credential: PurchaseReceipt
  anchor { … }
  require { signature.valid; status.active; holder.bound }
}
```

Used as `verify receipt with ReceiptFromMerchant`, so the same trust logic is
not written out at every point that needs it.

**A policy should name a trust anchor and let the chain answer, rather than
pinning an issuer identifier inside the application.** Pinning reintroduces at
application level exactly the allowlist that a chain-of-trust model removes at
host level — and it moves the decision into a place the person cannot see. Where
a host has no registry to resolve against, pinning is the escape hatch, and it
must be visible in the manifest rather than buried in the code.

### Selective disclosure

```
present {
  disclose member.country
  prove member.age >= 18
}
```

`Proof<bool>` means a real zero-knowledge proof. Where the host cannot yet
produce one, `prove` degrades to selective disclosure plus issuer-derived
claims — and **the compiler must refuse rather than silently weaken it.** A
language that quietly turns a proof into a disclosure has told the author
something untrue about what their application does.

---

## 3. Verification is type narrowing

`verify` and `Verified<T>` are **one mechanism, not two**. Passing a `verify`
block is the only way to obtain a `Verified<T>`; there is no cast, no
constructor, no runtime assertion that leaves the type unchanged.

A function may then demand `Verified<Employee>`, and unverified data will not
satisfy it. The check cannot be forgotten, because forgetting it does not
compile — which is a stronger statement than any amount of review discipline.

Provenance stays attached where practical, so a derived value knows what it came
from:

```
adult ← age ← AgeCredential ← { trusted issuer, valid signature,
                                valid status, holder binding }
```

---

## 4. Execution, and the record of it

```
VAL source → parser → AST → semantic analysis → type check →
capability analysis → trust analysis → IR → (evaluator | Wasm) →
host runtime → platform
```

### Determinism is a language property, not a runtime one

There is no `Date.now()`, no `random()`, no `fetch()` and no filesystem **in the
language**. Nondeterministic values arrive from the host as part of an explicit
runtime context — `context.time.now`, `context.random.uuid` — and are recorded
along with everything else.

An action is therefore a function:

```
(previous state, input, runtime context, code) → (new state, output, effects)
```

Replay, audit and state hashing all follow from that signature. Nothing else in
this document is worth much without it.

### Canonical encoding

State, input and code hashes need one canonical encoding, and it must be the
*same* one everywhere — a second canonicalisation is a second thing to get
subtly wrong. Deterministic CBOR (dCBOR) is the intended shape. A host that
already has one supplies it through the interface rather than the language
carrying a competing implementation.

### Package

A signed package: manifest, code, types, credentials, capabilities, assets,
runtime version, integrity, signature. It answers who published this, which
version is running, whether it was modified, what it may do, and what is
actually executing.

### Execution record

For actions that warrant one: application id and version, publisher, code hash,
action, input hash, previous and new state hashes, credentials and trust
policies used, capabilities used, effects requested and effects executed,
runtime context, timestamp, signature.

---

## 5. Compilation target

**No bespoke VM, now or later.** Everything a hand-built VM would provide —
sandboxing, resource limits, determinism, portability — Wasm already provides,
and maintaining one is a multi-year commitment to the part of the system that is
not the point.

**v1 does not compile at all.** It walks the typed AST in Rust. For expressions
this small that is a few hundred lines, it is deterministic, and it is by far
the easiest thing to instrument for an execution record.

**Wasm is the destination, reached when there is a reason to go there** —
untrusted third-party code needing hard fuel limits and signed bytecode. The
front end does not change; only the back end. Notes for whoever does that work:

- Wasm core has only `i32`/`i64`/`f32`/`f64`. Avoid the allocator problem by
  keeping values **host-side and passing `i32` handles**, with imported helpers
  (`val_field`, `val_add`, …). The compiler then emits calls and control flow
  and nothing else, which `wasm-encoder` makes mechanical.
- The no-floats rule in §2 exists mostly for this.
- **iOS forbids JIT**, so an interpreting runtime is required. `wasmi` or
  `wasm3` inside a Rust core covers iOS, Android and desktop from one
  integration; browsers come free. Check the state of those runtimes and of
  WasmGC before committing — this moves faster than a specification does.

---

## 6. User interface

**v1 has no `screen`.** An application is actions, trust policies and state, and
the host draws whatever it draws. This is not a placeholder for a missing
feature: a language that renders is a much larger language, and no application
has yet failed to be expressible without it.

When `screen` arrives, in ascending order of cost: a **host component catalogue**
with typed props · then **constrained layout primitives** (row, column, stack,
spacer) · **never arbitrary drawing.**

Constraints that hold regardless of when that happens:

- **Consent is host chrome.** The application cannot draw it, cover it, or
  imitate it.
- **Branding comes from the signed manifest**, not from free styling — otherwise
  one application dresses up as another.
- **UI is data, not code**, so it can be signed, diffed and audited with the same
  canonical encoding as the execution record.
- Host drawing earns accessibility, internationalisation and dark mode once,
  rather than every application breaking them separately.
- Declarative UI means tooling can preview an application without running it.

---

## 7. Open questions

Numbered so they can be answered one at a time.

1. **Are reads effects?** A credential lookup touches host state, can fail, and
   may prompt the person — but it is not a mutation, and forcing it into
   `execute` makes ordinary code awkward. *Recommended:* an action declares its
   data dependencies in a phase of its own rather than calling mid-computation.
2. **Capability granularity.** `credential.read` as a single capability lets a
   loyalty application read a passport, which is not least privilege in any
   meaningful sense. *Recommended:* parameterise —
   `credential.read(type: "LoyaltyMember")`, `credential.read(issuer: …)`.
3. **`update` then `execute` — what if the effect fails?** *Recommended:* the
   pure phases return `(newState, effects)`; the host runs the effects and
   commits the state only on success. The alternative is a state that records
   something that did not happen.
4. **What exactly does a host have to provide?** The interface is named
   throughout this document and specified nowhere. It needs writing before a
   second host is realistic, and writing it will be the honest test of whether
   the first host has leaked into the language.

---

## 8. Order of work

1. Parser and typed AST for the shell plus expressions.
2. Type checker: `Credential<T>`, `Verified<T>`, nullability, effect purity.
3. Capability and trust analysis over the typed AST.
4. Tree-walking evaluator, effect requests, execution records.
5. The host interface, and one capability wired end to end through a real host.
6. Everything else — Wasm back end, `screen`, packaging, proofs — after a real
   application exists and pushes on it.
