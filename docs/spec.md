# VAL

A language for Micro Apps — small applications that run inside somebody's
wallet, beside their passport and their bank credentials.

```val
app "th.co.codefin.loyalty"
version 1

capabilities {
  credential.read(PurchaseReceipt)
  credential.issue(LoyaltyMember)
}
```

A Micro App reads credentials the person already holds, computes, and asks the
wallet to act — issue a credential, take a payment, prove a fact.

You get four things without building them:

- **No login.** The person is already identified by credentials a government, a
  bank or an employer issued.
- **No user data to store.** It stays in their wallet. You read a claim under a
  policy you named.
- **Proofs without disclosure.** Ask whether somebody is over twenty without
  learning their birthday.
- **A signed record of every run**, to hand to a customer or an auditor.

## What is different from the languages you know

| | |
| --- | --- |
| Declare before you use | Everything the app can do is in `capabilities`. Using something you did not declare fails the build; declaring something you do not use fails the build too. |
| No floating point | Money is in minor units — satang, cents. Percentages are basis points. |
| No loops, no recursion | Lists are consumed by `map`, `filter`, `fold`, `any`, `all`, `count`, `first`. Every program halts. |
| No string building | No `+`, no interpolation. Every sentence a person reads comes from `text.json`, by key. |
| No screens of your own | You declare `card`, `row`, `button`; the wallet draws them. |
| No assignment | `const` only. Records are derived with spread; state is changed by an `update` block. |
| Errors are outcomes | No `Result`, no exceptions, no early return. An action commits or it does not. |

**New here?** Start with [your first application](guide/02-your-first-application.md),
which builds a working loyalty card. This document is the reference you come
back to.

---

## Program structure

A program is a **package**: one or more `.val` files plus a `text.json`. The
files share one scope — there are no imports and no per-file namespaces, so a
screen in one file may call an action declared in another.

```val
app "th.co.codefin.loyalty"     // reverse-DNS, in quotes
version 1

capabilities { … }              // what this app may do — declared once per package
enum · credential · type        // data declarations
state { … }                     // what persists between actions
trust … { … }                   // named verification policies
function … { … }                // pure helpers
action … { … }                  // the only executable thing
screen … { … }                  // what the person sees
```

### Syntax

- **A newline ends a statement.** No semicolons. A statement continues while a
  bracket is open, so wrap long expressions in `( … )`.
- **The shell is newline-separated, expressions are comma-separated.**
  `member.tier: tier` on its own line; `{ ...member, tier: tier }` with commas.
- **`if` and `switch` parenthesise their condition**, as in TypeScript and Dart.
- **Numbers are decimal**, with `_` allowed between digits: `100_000`. No hex,
  no binary, no exponents, and `12.50` is a compile error.
- **Identifiers are ASCII and camelCase** for names you choose. Claim names from
  an issuer keep their own spelling: `purchased_at`, `document_number`.
- **Arguments are named once there are two**: `payment.request(to: merchant,
  amount: 12000)`. One argument may be positional.
- **Keywords are reserved.** A dot is always field access.

---

## Types

| type | notes |
| --- | --- |
| `int` | 64-bit signed. Traps on overflow and on division by zero |
| `string` | compared and passed, never built |
| `bool` | `true`, `false` |
| `date`, `datetime` | compared as integers; add a `duration` to get the same type back |
| `bytes` | |
| `List<T>` | no index; use the combinators |
| `T?` | optional; narrow it with `exists` in `require` |
| `Credential<T>` | held but unverified — its claims are out of reach |
| `Verified<P>` | what `verify` produces. `P` is the **policy**, not the credential |
| `Proof<bool>` | what `prove` produces |

### Declaring data

```val
enum Tier { bronze, silver, gold }

credential PurchaseReceipt {      // signed by somebody else
  merchant:     string
  amount:       int               // satang
  purchased_at: datetime
}

type Quote {                      // a plain record; nobody signed it
  symbol: string
  price:  int
}

state {
  member:         LoyaltyMember?
  lifetimePoints: int default 0
}
```

State fields use `default`, not `=`. There is no assignment anywhere in the
language.

### Working with values

```val
const bumped = { ...member, points: member.points + earned }   // derive, never mutate
const fee    = amount > 100_000 ? 0 : 20                       // conditional expression

const discount = switch (tier) {
  Tier.bronze => 0,
  Tier.silver => 5,
  Tier.gold   => 10,
}
```

A `switch` over an enum **may not have a `default`**, so adding `Tier.platinum`
breaks every program that decides something per tier. A `switch` over `int` or
`string` requires one. An arm that can never be reached is an error.

---

## Credentials and trust

Every credential has the same four faces:

```val
receipt.claims        // what the issuer said — your declared fields
receipt.signature     // .valid
receipt.status        // .active
receipt.holder        // .bound — is this the person in front of us
```

The last three are readable only inside `trust` and `verify`.

### Write a policy

```val
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

`anchor` names a root the certificate chain resolves against, so adding a
merchant to your scheme does not mean shipping a new version of your app.

Put freshness in the policy. A valuation from last week is signed, unrevoked and
correctly bound — and wrong:

```val
holding.claims.valued_at > context.time.now - duration(hours: 24)
```

### Use it

```val
verify {
  const checked = receipt with ReceiptFromMerchant
}

compute {
  const earned = checked.claims.amount / 100
}
```

`checked` has type `Verified<ReceiptFromMerchant>`. There is no cast that
produces one and no way to reach `.claims` without it.

**The type names the policy.** `Verified<SignatureOnly>` and
`Verified<ReceiptFromMerchant>` are different types, and a function that wants
the second will not take the first. If one policy really does subsume another,
declare it:

```val
trust StrictReceipt(r: PurchaseReceipt) refines ReceiptFromMerchant { … }
```

### Provenance

Every value remembers which policies it descends from, and the compiler
propagates that. You write it in one place — on a claim you issue:

```val
credential.issue(LoyaltyMember {
  points: next.member.points from { ReceiptFromMerchant }
})
```

`from` is a requirement: this claim may be computed only from data verified
under that policy. Mix in anything else and it does not compile. Whoever
receives the credential can then check how the number was reached instead of
taking your signature's word for it.

---

## Actions

An action is the only executable thing:

```
(previous state, input, runtime context, code) → (new state, output, effects)
```

```
input → require → verify → compute → update → execute
```

Omit any phase; never reorder them.

| phase | what goes in it |
| --- | --- |
| `input` | what the action is given |
| `require` | preconditions, and narrowing `T?` with `exists` |
| `verify` | trust policies |
| `compute` | pure calculation, and `refuse` |
| `update` | the next state, as a patch |
| `execute` | effects — the only phase where any may appear |

```val
action ScanToEarn {
  input {
    receipt: Credential<PurchaseReceipt>
  }

  require {
    state.member exists
  }

  verify {
    const checked = receipt with ReceiptFromMerchant
    checked.claims.purchased_at > context.time.now - duration(days: 30)
  }

  compute {
    if (checked.claims.amount < 2_000) { refuse "tooSmallToEarn" }

    const earned = checked.claims.amount / 100
    const total  = state.lifetimePoints + earned
  }

  update {
    lifetimePoints: total
    member.points:  state.member.points + earned
  }

  execute {
    credential.issue(LoyaltyMember {
      member_id: next.member.member_id,
      points:    next.member.points,
    })
  }
}
```

Names from `input` are in scope everywhere after it, bare. The prefixed roots
are `state.`, `context.`, and `next.` inside `execute`.

### The four ways an action does not commit

Pick the right one — this is the most common mistake in a first app.

| | who sees it | use it for |
| --- | --- | --- |
| `require` fails | nobody | something that should never be false. If it is, you have a bug |
| `verify` fails | the person | a forged, expired or out-of-anchor credential |
| `refuse "key"` | the person | your own rule: too small, too soon, already claimed |
| the host refuses | the person | they said no |

`refuse` takes a key from `text.json`, never a sentence, and must appear before
`execute`.

### `update` is a patch

```val
update {
  lifetimePoints: total
  member.points:  state.member.points + earned
}
```

Each line is `path: value`; anything unnamed is unchanged. Paths may nest but
may not contain a list index — build a new list in `compute` and name it here in
one line. The result is bound as `next` for `execute` to read.

### `execute` is one batch

The host takes every effect or none, and your state commits only if it took
them. No effect can read another's result: if one depends on another's outcome,
that is two actions.

- **At most one disclosure per action.** A disclosure cannot be undone, so a
  second one could not depend on a batch the first has already completed. Two
  disclosures need two consents, which means two actions.
- **Irreversible effects run last.** The compiler orders them for you.

### Functions are pure

```val
function tierFor(points: int): Tier {
  return switch (points) {
    >= 10000 => Tier.gold,
    >= 2000  => Tier.silver,
    default  => Tier.bronze,
  }
}
```

There are no effectful functions, so an effect cannot hide behind a call.

Note: a sequence of effects used by three actions has to be written out three
times. In exchange, everything an action can do is in its `execute` block, with
no call graph to follow.

---

## Screens

You declare a screen; the wallet draws it.

```val
screen Wallet {
  data {
    receipts: credentials of PurchaseReceipt verified with ReceiptFromMerchant
      order by purchased_at desc
      limit 50
  }

  compute {
    const totalValue = receipts.fold(0) { sum, r -> sum + r.claims.amount }
  }

  column {
    card(text: phrase("balance", points: state.member.points))
    section(text: "history")
    list(receipts) { r ->
      tile(text: phrase("receiptLine", merchant: r.claims.merchant, at: r.claims.purchased_at))
    }
    button(text: "scan", emphasis: primary, onTap: ScanToEarn)
  }
}
```

**Declare data; do not fetch it.** The host resolves the `data` block before
anything is drawn. `verified with` means a credential that fails the policy
cannot appear — not "is filtered out".

`limit` is required on a list you compute over. It bounds the work, which is
what lets a total over the list compile to a circuit.

**A press names an action.** `onTap` is the only kind of handler, so everything
a screen can start goes through the six phases with the same consent and the
same record.

**A screen may derive, and may not act.** Its `compute` block follows an
action's rules: pure, no effects.

Note: keep totals in `compute`, not in `state`. A value you can compute from
what is already on the screen does not need to be stored, hashed and replayed.

**Interaction state belongs to the host.** Which tab is open, scroll position,
what is typed but not submitted. An action receives what the form held at the
moment it was submitted, through `input`.

### Components

What the wallet ships, not what the language defines:

```val
column { … }
section(text: "key")
card(text: phrase("key", name: value))
tile(text: phrase("key", name: value), onTap: Action)
list(binding) { item -> … }
button(text: "key", emphasis: primary, onTap: Action)
```

Props are semantic — `text`, `icon`, `emphasis`, `state`, `onTap`. No colours,
no fonts, no pixel sizes. Asking for a component the catalogue does not have is
reported, not approximated. Your package records the catalogue version it was
built against, and a host renders those semantics or refuses to run it.

### Text

Write the words. An application in one language needs no bundle at all:

```val
section(text: "Your receipts")
card(text: phrase("You have {points} points", points: state.member.points))
```

`phrase` carries the values; the host formats numbers, dates and currency for
the language it is running in.

A second language turns those words into keys:

```json
{
  "locales": ["en", "th"],
  "keys": {
    "balance": { "en": "You have {points} points", "th": "คุณมี {points} แต้ม" }
  }
}
```

```val
card(text: phrase("balance", points: state.member.points))
```

A package that promises two languages and writes words in place is a failed
build, naming the language they would be wrong in. So is a missing slot, a slot
of the wrong type, or a key one language does not translate.

### Data that is not a credential

Prices, news, a catalogue. Fetch it through the host:

```val
data {
  holdings: credentials of Holding verified with FromLicensedBroker
  prices:   query broker.quotes(symbols: holdings.symbols) as List<Quote>
}
```

Your app authenticates by presenting a credential and **never touches the
token**. The host presents, gets the access token, makes the request and returns
the rows.

- The audience is fixed in your manifest, never built at runtime.
- Getting access is a disclosure: declare `disclosure.present`.
- The person consents once per app and audience, not per refresh.
- The host caches the answer and displays its age.
- A failure tells you the query did not answer, not why.

The wallet shows three grades of data differently, and you cannot choose how:

| grade | source |
| --- | --- |
| issuer-backed | a claim in a credential |
| self-asserted | your own `state` |
| origin-asserted | a query answered by an authenticated API |

---

## Disclosing and proving

```val
execute {
  present {
    disclose checked.claims.country
    prove checked.claims.birthdate <= context.time.now - duration(years: 20)
  }
}
```

`disclose` hands over a value. `prove` hands over an answer — the verifier
learns that somebody is over twenty and cannot work out the birthday.

`prove` produces a `Proof<bool>` and nothing weaker. Where a real zero-knowledge
proof cannot be produced, **your app does not build**; it never falls back to
disclosing and comparing.

### What can be proved

`prove` compiles to a circuit, and only part of the language does. The compiler
tells you when you leave it.

Inside: integers with a declared width (`int<32>`), dates and times compared as
integers, string equality, `switch` and `?:`, list combinators with a
compile-time length, a nullifier computed from the holder's secret, and Merkle
inclusion against the state root.

Outside: effects, and anything whose size is not known statically.

Two things to know before writing one:

- **Every branch costs.** A circuit pays for both sides of a conditional.
- **It pays for the bound, not the data.** A proof over a list of at most 200
  costs 200 additions whether the person holds two positions or two hundred —
  which is also why it does not leak how many they hold.

### Proving things about your own state

State is a Merkle tree, so one field can be shown without opening the rest:

```val
disclose state.member.tier
prove state.lifetimePoints >= 10_000
```

Know what you are claiming. A credential claim is backed by an issuer's
signature. A state field is backed by the chain of records that produced it —
correct by rules anyone can re-run, but with no third party behind the input.
The verifier is told which it is looking at.

**Anything from an API cannot be proved.** A query answer is somebody's word,
not somebody's signature. The compiler refuses it; disclose the number and say
where it came from.

---

## State and the execution record

`state` is yours, on their device, changed only by `update`.

After every action the host builds a Merkle tree over your state's
`(path, value)` leaves and records the **root**. The next record carries the
previous root, so the two chain.

This is tamper-**evident**, not tamper-proof: the state is on somebody's device
and they can discard it. What the chain gives is detection, to anyone who kept
an earlier record — which is why a verifier remembers the last root it saw, and
why an issued credential records the root it was derived from.

A credential is spent once. Rolling back and rescanning the same receipt is a
double-spend, and the issuer refuses it by recording a **nullifier** — computed
inside the proof from the holder's secret and the scheme's identifier, so it is
the same value every time for that pair and unrelated for any other.

### What a record contains

App id and version, publisher, code hash, action, input hash, previous and new
state roots, policies used, capabilities used, effects requested and executed
(disclosures among them), runtime context, timestamp, signature.

### Keep state small

- **No derived values.** Screens have `compute`.
- **No interaction state.** That is the host's.
- Sizes are bounded by the host, and the limit is checked before the state
  commits.

### Changing the shape is a new version

**A change to the shape of `state` starts that version's state empty.** There is
no migration, no compatibility shim and no dual reader — a migration is code
that runs against data the current version never produced and cannot be replayed
from a record, because no action performed it.

Anything you cannot afford to lose belongs in a credential you issued, or on
your own backend.

---

## Determinism

There is no `Date.now()`, no `random()`, no `fetch()` and no filesystem in the
language. Nondeterministic values arrive from the host and are recorded:

```val
context.time.now      context.random.uuid
```

A query answer that crosses into `compute`, `update`, `issue` or `prove` is
recorded in the runtime context too, so the run can still be replayed.

Every program halts: the call graph must be acyclic, and lists are consumed by
bounded combinators only.

---

## Packaging

```bash
valc    file.val …             # diagnostics, then the capability report
valrun  file.val ActionName    # run one action, print the execution record
valpack build  ./dir -o app.va
valpack verify app.va
```

A `.va` is one signed document: your sources, the manifest, the text bundle, the
derived capability report, a hash per file, and a signature over all of it. The
same inputs produce the same bytes.

**The sources travel in the package**, so the host can check it from first
principles rather than trusting your build.

### The capability report

The compiler derives it from your code; you cannot write or edit it.

```
reads          PurchaseReceipt.amount, PurchaseReceipt.purchased_at
               under ReceiptFromMerchant
discloses      NationalId.country
proves         birthdate <= now - 20 years
issues         LoyaltyMember
talks to       broker.co.th
writes state   member.points
irreversible   one disclosure
```

The consent sheet the person approves is a rendering of this report. Read it as
they would: if it says something you did not intend, your code says it.

### What the host checks before admitting your package

1. Every source hashes to what integrity says.
2. The signature is over these bytes, by the key your manifest names.
3. It compiles — checked there, not taken from your build.
4. The report it ships is the report its code produces.
5. Every locale your manifest promises has every key.

Then its own policy: whether an app of your kind may hold those capabilities,
and whether it can render your catalogue version.

### Who signs the credentials you issue

Not your app. It has no issuer key and must not have one.

```
device        runs the action, signs the execution record
   ↓
your backend  verifies that record, signs the credential with your issuer key
   ↓
device        stores the credential
```

Your server does not run VAL, hold state, or see it. It checks the signature,
resolves the code hash to a version you published, checks the trust chains,
verifies any proof — the verification key comes from the compiled circuit, so it
knows which predicate was proved — and then signs, or refuses.

Somebody holding the device can rewrite their own state. They cannot make your
server sign a credential for a run that does not verify.

---

## How it runs

```
.val sources
     │
     ├─ lexer → parser → typed AST
     ├─ type checking          Verified<P>, T?, provenance
     ├─ capability and trust analysis
     ├─ determinism and totality
     │
     ├─ evaluator              walks the typed AST
     └─ Wasm back end          for hard fuel limits and signed bytecode
```

Both back ends read the same typed AST; there is no intermediate representation
between them. The Wasm back end keeps values host-side and passes `i32` handles,
so no allocator is needed, and trapping integer overflow maps to Wasm traps
directly. iOS forbids JIT, so the runtime interprets.

### What the host supplies

VAL has no I/O. Where an effect is called for it emits a description:

```
EffectRequest { capability, operation, payload }
```

The host decides, in order: is the capability declared · did the person consent ·
does host policy allow it · is the app trusted · is the operation in scope.

It also supplies the canonical encoding (deterministic CBOR), the clock and
randomness, trust resolution, the component catalogue, session and token
handling, and the bounds on value sizes.

---

## Reference

### Builtins

A closed set. An application cannot add to it.

```val
duration(days: 30)  duration(hours: 24)  duration(years: 20)
min  max  abs
```

### List combinators

```val
map  filter  fold  any  all  count  first
```

### Effects

Only in `execute`, never behind a function, offered as one batch.

```val
credential.issue(Type { … })
payment.request(to: …, amount: …)
storage.write(scope: …, id: …, value: …)
message.send(to: …)
network.request(…)
present { disclose … / prove … }
navigate Screen
```

### Expressions

```val
const x = …                      // no var, no assignment
a ? b : c                        // if is a statement; this is the expression
if (cond) { … } else { … }
switch (x) { A => 1, B => 2, }   // no default over an enum
{ ...record, field: value }      // derive; never mutate
x with Policy                    // the only way to get Verified<P>
x exists                         // narrowing, in require
value from { Policy }            // provenance, on an issued claim
```

### Screen data

```val
data {
  name: credentials of Type verified with Policy
    order by field desc
    limit 50

  other: query audience.operation(…) as List<Type>
}
```
