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
| Every program halts | No recursion, and no loop whose end is not known: lists are consumed by `map`, `filter`, `fold`, `any`, `all`, `count`, `first`, and a `for` over a screen runs over a list the host answered with or a range whose length is written down. |
| No string building | Sentences are not concatenated. `` `you have ${points} points` `` is a phrase, and the host fills and formats it — which is how one language's numbers are right in every application at once. |
| No screens of your own | You declare `card`, `row`, `button`; the wallet draws them. |
| State is changed by an `update` block | Not by assignment. A local `let` may be written again; `state` may not, and a record is derived with spread. |
| Errors are outcomes | No `Result` and no exceptions. An action commits or it does not. |

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
admits { … }                    // who it opens for at all — optional, declared once
enum · credential · type        // data declarations
state { … }                     // what persists between actions
trust … { … }                   // named verification policies
function … { … }                // pure helpers
action … { … }                  // the only executable thing
screen … { … }                  // what the person sees
```

### Syntax

- **A newline ends a statement.** No semicolons. A statement continues while a
  round or square bracket is open, so wrap long expressions in `( … )`. A brace
  holds statements, so a newline inside one separates rather than continues —
  which is why a `switch` arm ends at its line and its comma is optional.
- **The shell is newline-separated, expressions are comma-separated.**
  `member.tier: tier` on its own line; `{ ...member, tier: tier }` with commas.
- **`if` and `switch` parenthesise their condition**, as in TypeScript and Dart.
- **Numbers are decimal**, with `_` allowed between digits: `100_000`. No hex,
  no binary, no exponents, and `12.50` is a compile error.
- **Identifiers are ASCII and camelCase** for names you choose. Claim names from
  an issuer keep their own spelling: `purchased_at`, `document_number`.
- **Arguments are named once there are two**: `payment.request(to: merchant,
  amount: 12000)`. One argument may be positional.
- **A keyword is never a name**, so reading a declaration never depends on
  knowing what else the package declared. Claim names from an issuer are not
  affected: they are read from the credential, not chosen here. A dot is always
  field access.
- **A directive marks a declaration; a setting configures one.** `@main` is
  written on its own line above a `screen`. A directive may take arguments —
  `@name(value)` — and `@main` takes none. The set of directives belongs to the
  language, not to a host: which screen a package opens at is the same fact in
  every wallet that runs it. Settings — `present:`, `address:` — take their
  values from the host's vocabularies instead.

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

State fields use `default`, not `=`: state is changed by an `update` block, so
that every change to it is a line in the record somebody can read. A local `let`
is a different thing and may be written again.

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

### Who the application opens for

Most applications open for anybody holding the phone. Some do not:

```val
admits {
  EmployeeBadge with EmployedByAcme else "notStaff"
}
```

Without a credential that passes `EmployedByAcme`, this application does not
draw its first screen and does not run an action. The person is shown
`notStaff` — a key in the text bundle, so the words are the publisher's,
reviewed and translated like every other sentence in the package.

**The host answers the door, and the program never asks.** There is no way to
write "does this person hold one?" in VAL, and the omission is the design: a
program that could ask would be holding the credential in order to discover it
was absent, which is exactly the read the gate exists instead of. What an
admitted application learns is that it opened.

So the gate is a `credential.check`, never a `credential.read` — the difference
the consent sheet turns into two different sentences about the same
application. Declare it:

```val
capabilities { credential.check(EmployeeBadge) }
```

Both halves of a line are required. Without the policy, a gate admits anything
shaped like a badge, including one somebody made; without the sentence, a door
closes in silence and the person is left with a fault report instead of an
instruction. Every line must pass — two gates are two conditions and never a
choice, because a door that opened for either of two credentials is one policy
over one credential, written where a reviewer can see that is what it is.

The gate is the one thing on the consent sheet a host **evaluates** rather than
**measures**, so what keeps it honest is [the check below](#what-the-host-checks-before-admitting-your-package):
the credential it names has to be one the module's own import section says this
application looks at. A door on the sheet and no door in the code is refused as
a sheet that is wrong.

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

Every line reads the state the action started with, because the block is one
patch rather than a sequence of assignments. A swap is what tells the two apart:

```val
update {
  a: state.b
  b: state.a
}
```

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
@main
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

**One screen carries `@main`.** It is where the application opens; every other
screen is reached by a press. A package with any screen and no `@main` is
rejected — one screen included — because otherwise the first screen somebody
sees would depend on the order the package's files were read.

**Imports are resolved through.** A package may export something built out of
what it imported, and a circle among packages is reported rather than expanded:
neither half of it is a cycle on its own, so the check is made where they meet.

**A screen may show one tree or another.**

```val
if (state.points > 0) {
  card(text: phrase("balance", points: state.points))
} else {
  emptyState(text: "notAMember")
}
```

`else` is optional. Both branches are checked, and both contribute to the
capability report: a capability used only in the branch that is not taken today
is one the person consented to. The condition is resolved before anything is
drawn, so a host receives a tree with no condition in it and needs no `if` of
its own.

**Declare data; do not fetch it.** The host resolves the `data` block before
anything is drawn. `verified with` means a credential that fails the policy
cannot appear — not "is filtered out".

`limit` is required on a list you compute over — `map`, `filter`, `fold`, `any`
or `all`; `count` and `first` read it without walking it. It bounds the work,
which is what lets a total over the list compile to a circuit, and what makes a
proof cost the bound rather than the data.

**A press names an action.** Every handler does — `onTap`, and `onRemove` on a
list — so everything a screen can start goes through the six phases with the
same consent and the same record. Which props are handlers is the host's
registry's answer, not a name written into the compiler.

**A screen may derive, and may not act.** Its `compute` block follows an
action's rules: pure, no effects — a credential issued while a screen was being
drawn is one nobody pressed anything to get.

Note: keep totals in `compute`, not in `state`. A value you can compute from
what is already on the screen does not need to be stored, hashed and replayed.

**Interaction state belongs to the host.** Which tab is open, scroll position,
what is typed but not submitted. An action receives what the form held at the
moment it was submitted, through `input`.

### When something may not be there

```val
state.member?.points          // nothing, if there is no member
state.member?.points ?: 0     // nothing becomes zero
```

An `?.` anywhere along a path stops the whole path: `a?.b.c` is nothing when `a`
is, rather than a failure at `c`. Reading a field of nothing is a defect. An application that wrote
`state.member.points` wrote it believing there was a member, and answering with
nothing again is how that belief reaches a person as a blank card. `?.` says
there might not be one, and makes the whole path optional; `?: ` supplies what
to use instead. Neither evaluates its left side twice.

### Taking a record apart, and leaving an argument out

```val
const { merchant, amount } = row

component Badge(label: string, tone: string default "neutral") { … }
Badge(label: "one")
```

Destructuring is one statement rather than one binding per field, so the
right-hand side is read once — a record here can be a credential the host had to
be asked for. Every field keeps the record's provenance: pulling `amount` out of
a verified receipt does not make it a number somebody typed.

`default` is the same word a state field uses, and means the same thing: what a
value is when nobody supplied one. It is written where the parameter is, so it
is written once rather than at every call site.

### A bare name on a screen

It is one of three things, and a fourth is a mistake:

- something the program declares — state, a screen's `data`, a computed value, a
  parameter, the row a loop reads;
- a word the host's registry has — `primary`, `money`, `foreground.primary`. A
  prop whose type is an open vocabulary takes a word of the application's own,
  because a token is guidance rather than a fence;
- the name of an action or a screen, where the prop holds one — `onTap:`, and
  `into:`, which calls the field the host will keep what is typed under.

Anything else is refused. It used to be drawn as itself, so a misspelt binding
reached the screen as the word somebody typed.

### Across packages

A component is visible to every file in its package — a package's files share
one scope, so the boundary `export` crosses is the package, never the file.

```val
export component MoneyCard(label: string, amount: string) { … }
```

```val
import "org.vaulet.ui/1" { MoneyCard }
```

An external thing is named the way every external thing in this language is
named: a quoted identifier with a version, as `host "id.vaulet.wallet/1"` is. A
package is a signed artifact rather than a namespace, so what is imported from is
a version and not a scope. The names are listed rather than opened wholesale:
everything that crossed into a package is signed as part of it, and one line
says what that was.

**Imports are resolved at build time.** The imported component is expanded in the
package that wrote it and then folded into the importer, so the package a host
admits is one program. There is no linking step and nothing resolved at run
time. A private helper of the exporting package comes along without its name and
cannot collide with the importer's own.

**What an import draws is declared by the package that draws it.** An imported
component needing `media.video` needs it in the importing package's
`capabilities` block. A person consents to one list.

**An exported component reads its own parameters and nothing else** — not
`state`, `input` or `context`, which belong to whichever package it is expanded
into. Text keys inside it are looked up in the importing package's bundle, for
the same reason: the words belong to the application somebody is looking at.

**A version is what an importer depends on.** Changing an exported component's
parameters is a breaking change to packages that are not yours, so it is a new
version rather than an edit.

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
no fonts, no pixel sizes. Asking for something this host does not provide is
reported, not approximated. Your package records the registry version it was
built against, and a host provides those semantics or refuses to run it.

### Text

```val
text(`you have ${state.points} points`)
```

A `` ` `` string is sugar for `phrase`: the line above is exactly

```val
text(phrase("you have {points} points", points: state.points))
```

The words and the values travel to the host separately, because the host formats
the number. A slot takes the last segment of the path it holds, so a bundle for a
second language reads as a sentence; a name used twice, or an expression that is
not a path, takes its position instead. Any expression may go inside `${…}`, and
means there what it means anywhere else.

Words written in place are still words: a package promising two locales is
refused for a `` ` `` string exactly as it is for a `"` one. Interpolation is
for the application in one language, which most are.

Write the words. An application in one language needs no bundle at all:

```val
section(text: "Your receipts")
card(text: phrase("You have {points} points", points: state.member.points))
```

`phrase` carries the values; the host formats numbers, dates and currency for
the language it is running in.

A phrase's values sit beside the node's own arguments, so they share one set of
names: a slot called `color` on a node that has a `color` is refused, and so are
two phrases on one node that both say `{n}`. Every prop the registry says holds
a sentence is checked, not the one called `text`.

A key the bundle holds and nothing reads is said rather than refused. An unused
capability is consent somebody gave for nothing; an unread sentence is one
somebody translated for nothing, which is worth knowing and not worth stopping
for.

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

A state field's `default` is a value written out. It is read before anything has
run: there is no scope to read a name from, and no previous version to ask.

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
valpack build  ./dir -o app.vapp
valpack verify app.vapp
```

A `.vapp` is one signed document: **the compiled module**, the manifest, the text
bundle, the derived capability report, the module's hash, and a signature over
all of it. The same inputs produce the same bytes.

**No source travels in the package.** A wallet has no compiler — compiling on a
phone is not a thing anybody ships — so what it is handed is what runs, and
every check it makes is a check on those bytes. It still takes nothing on
trust: what an application can do is the module's **import section**, which a
wallet reads off the bytes in linear time, and a module can reach only what it
imports.

That a module is the source you published is answered by **reproducible
builds**, not by shipping the source to every phone: the same source and the
same compiler produce the same bytes, so anybody who cares builds it once and
compares.

### The capability report

The compiler derives it from your code; you cannot write or edit it.

```
reads          PurchaseReceipt.amount, PurchaseReceipt.purchased_at
               under ReceiptFromMerchant
checks         NationalId under GovernmentIssued
discloses      NationalId.country
proves         birthdate <= now - 20 years
issues         LoyaltyMember
talks to       broker.co.th
writes state   member.points
exports        MoneyCard(label: string, amount: string)
imports        org.vaulet.kit/1 { MoneyCard }
irreversible   one disclosure
```

The consent sheet the person approves is a rendering of this report. Read it as
they would: if it says something you did not intend, your code says it.

`exports` is the surface other packages build against, and the last two lines
are the only ones that are not about the person: they are for whoever depends on
this package. Nothing can check that a change to an exported component came with
a new version — the packages that depend on it are not present at your build,
and their authors are not there to say a parameter moved. What is present is
what this package exported last time, if it was kept: `valc --surface <file>`
keeps it and refuses a changed surface at an unchanged version.

### What the host checks before admitting your package

1. The module hashes to what integrity says.
2. The signature is over these bytes, by the key your manifest names.
3. It imports **only what that host provides** — a name the host does not know
   is a refusal rather than a link, because otherwise the list of what it can do
   would stop being the whole of what it can do.
4. The report it ships is the report its module produces.
5. The module and the manifest name the same application at the same version.
6. Every gate it states is one its module carries the `check` import for, and
   the sentence a closed door shows is in the bundle.
7. Every locale your manifest promises has every key.

Then its own policy: whether an app of your kind may hold those capabilities,
and whether it provides the registry version you built against.

### What a dishonest publisher cannot do, and what they can

You compile your own module and sign it, so nothing about it having come from a
VAL source can be assumed. Every check is a check on the bytes.

**Cannot: reach anything the sheet does not name.** What a module can do is what
it imports, and a name the host does not know is a refusal rather than a link.
There is no memory it shares, no syscall and no dynamic linking.

**Checking is not reading, and the sheet says which.** `verify` hands a module a
credential checked against a policy; whether it reads anything off it is a
separate fact, and the two are separate lines. An application that checks your
national ID against the government's key, discloses your country and proves your
age **reads nothing**, and the sheet says so.

**Cannot: hold a claim the sheet does not name.** A credential is handed over
carrying only the claims that were imported by name — however it arrives,
through `verify` or through a declared input. Asking for the credential and one
claim of it used to hand over all of them, and the sheet would have said one:
true about the import list and false about what the module held.

**Cannot: disclose something other than what the sheet names.** `disclose` takes
nothing. The host fetches the claim the import names and hands it to whoever is
being shown it, so the module never holds it — it cannot substitute one, keep
it, or compute on it. And a disclosure is not a read: an application that
discloses your country has not read your country.

**Cannot: compose the request a `present` sends.** Its lines are gathered by the
host as they are called. A module that could hand over the list could hand over
one that says anything.

**Cannot: sign a record that understates.** The capability list in every record
is measured from the import section, not read from the metadata beside the code.

**Cannot: put a door on the sheet that is not in the code.** Who an application
opens for is the one thing on the sheet a host evaluates rather than measures —
"you do not hold one" is an answer only a wallet can give. So a stated gate has
to name a credential the module's import section says this application looks at.
Overstating it that way is caught; understating it costs the publisher their own
gate and nobody else anything.

**Cannot: hang the phone.** Totality is a property of programs this compiler
emits, and yours may not be one, so a fuel budget is always set.

**Cannot: ship a report that says less than the module does.** It is derived
again on the device and compared.

**Cannot: name a number on the sheet and hand the host another.** A payment's
amount is computed, so the report does not name one: it names who the money goes
to, which is written out and cannot be computed. The amount is shown by the host
at the moment, out of the request itself, in a sheet the application cannot
draw. A person consents twice — once to the ceiling, once to the number.

**Cannot: be somebody else, when the name carries the key.** `did:key:z6Mk…` is
the public key written as a name, so `owns_key` is a comparison and needs
nothing fetched and nobody trusted. A package signed by another key is refused
by the default policy.

**Can: be somebody else, when the name has to be looked up.** `did:web:some.bank`
needs a document from a server, which is I/O and so is the host's. The default
policy refuses such a package rather than admitting one whose publisher is a
claim; a host that resolves the name says so by implementing `owns_key`. **A
host that neither resolves nor refuses is a host where any publisher can be any
publisher.**

**Can: ship a module that is not the source you published.** Nothing on the
device can tell, because nothing on the device compiles. That is what
reproducible builds are for — `valpack reproduce <dir> <app.vapp>` builds the
source and compares the bytes, and the module names the compiler that made it so
the comparison is between like and like. It is a check somebody does once,
rather than one every phone does at every install.

**Can: say different things in different languages.** The bundle is signed and
checked for missing keys and missing slots; whether the Thai and the English
mean the same thing is not something a compiler can answer.

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
                 the publisher's machine          │        the device
                                                  │
.val sources                                      │   .vapp  (module + manifest
     │                                            │        + text + signature)
     ├─ lexer → parser → typed AST                │        │
     ├─ type checking      Verified<P>, T?, …     │        ├─ the bytes hash right
     ├─ capability and trust analysis             │        ├─ the publisher signed them
     ├─ determinism and totality                  │        ├─ its imports are the report
     │                                            │        └─ run it
     ├─ evaluator          walks the typed AST    │
     └─ Wasm back end   ───────────── module ─────┼────────→ wasmi
```

Both back ends read the same typed AST; there is no intermediate representation
between them. The Wasm back end keeps values host-side and passes `i32` handles,
so no allocator is needed, and trapping integer overflow maps to Wasm traps
directly. iOS forbids JIT, so the runtime interprets — which is also why nothing
on a device compiles anything.

The module carries what a record needs beside the code: which application, which
version, the capabilities declared, the policies, the credentials each action
asks for, and the state's declared defaults. A wallet fills in an execution
record from the bytes and nothing else.

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

The function is written where it is used, or named:

```val
receipts.map { r -> r.claims.amount }
receipts.map(amountOf)
receipts.fold(0, add)
```

A named function takes what the combinator hands over — one value, or two for
`fold`, whose first is the running one.

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
const x = …                      // a definition
let x = …                        // a variable; `x = …` writes it again
a ? b : c                        // if is a statement; this is the expression
a ?: b                           // a, unless it is nothing
a?.b                             // nothing, if a is
if (cond) { … } else { … }
switch (x) { A => 1, B => 2 }    // no default over an enum
{ ...record, field: value }      // derive; never mutate
{ a, b } = record                // after `const` or `let`
0...10                           // both ends included
`words ${value} words`           // a phrase, filled by the host
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
