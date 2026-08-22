# VAL — the language, in one page

A small, total language for **applications a wallet runs on somebody's phone**.
Written against the compiler as it stands on 2026-08-22; every construct below
appears in an example under `examples/` that compiles and is tested.

---

## 1. What makes it different, in one idea

**What an application can do is derived from its code, not declared beside it.**

VAL compiles to **WebAssembly**. The consent sheet a person reads is the
module's **import section**: a module can call what it imports and nothing else,
so a report cannot understate what an application reaches for. Editing metadata
does not change it — there is nothing to edit.

```
reads          PurchaseReceipt.amount, PurchaseReceipt.purchased_at
               under ReceiptFromMerchant
checks         EmployeeBadge under EmployedByAcme
discloses      NationalId.country
proves         birthdate <= now - 20 years
issues         LoyaltyMember
talks to       broker.co.th
writes state   member.points
irreversible   one disclosure
```

Everything else follows from wanting that sentence to be true.

---

## 2. The shape of a package

One or more `.val` files plus a `text.json`, sharing **one scope** — no
per-file namespaces, and one `app`, `version` and `capabilities` block across
all of them.

```val
app "th.co.codefin.loyalty"     // reverse-DNS, quoted
version 1

host "id.vaulet.wallet/1"       // optional: which registries this needs
import "org.vaulet.ui/1" { MoneyCard }   // components from another package

capabilities { … }              // what this app may do — once per package
admits { … }                    // who it opens for at all — optional
enum · credential · type        // data
state { … }                     // what persists between actions
trust … { … }                   // named verification policies
function … { … }                // pure helpers
action … { … }                  // the only executable thing
screen … { … }                  // what the person sees
component … { … }               // a reusable arrangement of the host's catalogue
```

---

## 3. Data

```val
enum Tier { bronze, silver, gold }

// A credential says the type its issuer stamps — required, because
// `PurchaseReceipt` is a name this package chose and no wallet has heard it.
credential PurchaseReceipt as "https://org.vaulet.id/codefin/credential/purchase-receipt" {
  merchant:     string
  amount:       int        // satang. There is no float in this language
  purchased_at: datetime
}

type Quote {               // a plain record; nobody signed it, so no `as`
  symbol: string
  price:  int
}

state {
  member:         LoyaltyMember?
  lifetimePoints: int default 0
}
```

Types: `string int bool date datetime bytes`, `List<T>`, `Credential<T>`,
`Verified<Policy>`, `Proof<T>`, and `T?` for optional.

**No floats.** Money is satang, shares are micro-shares, percentages are basis
points. Arithmetic **traps on overflow** and on division by zero.

**The URL is the issuer's own** — absolute and `https` and nothing more is
checked. A self-hosted issuer writes their own domain.

---

## 4. Trust policies

```val
trust ReceiptFromMerchant(receipt: PurchaseReceipt) {
  anchor: "th.co.codefin.merchants"   // an anchor, not a pinned issuer

  require {
    receipt.signature.valid
    receipt.status.active
    receipt.holder.bound
    receipt.claims.amount > 0
  }
}
```

The subject is bound by name (`receipt.signature.valid`, never a bare
`signature.valid`). `x with Policy` is **the only way** to obtain
`Verified<Policy>` — and the type names the *policy*, not the credential, so
data checked strictly and data checked loosely cannot share a type.

---

## 5. Who the application opens for

```val
capabilities { credential.check(EmployeeBadge) }

admits {
  EmployeeBadge with EmployedByAcme else "notStaff"
}
```

Without a credential passing that policy, the application draws no screen and
runs no action. The person reads `notStaff` — a key in the signed text bundle.

**The host answers the door and the module is never instantiated.** There is
deliberately no way to write "does this person hold one?": a program that could
ask would be holding the credential in order to discover it was absent, which is
the read a gate exists instead of. So it is `credential.check`, never
`credential.read` — the application is told it opened, never what opened it.

Both halves are required. Every line must pass; two gates are two conditions and
never a choice.

---

## 6. Actions — the only executable thing

Six phases, always in this order, each with one job:

```val
action ScanToEarn {
  input   { receipt: Credential<PurchaseReceipt> }

  require { state.member exists }          // defects. Nobody is shown a message

  verify  {                                // trust. Failing is ordinary
    const checked = receipt with ReceiptFromMerchant
    checked.claims.purchased_at > context.time.now - duration(days: 30)
  }

  compute {                                // pure
    if (checked.claims.amount < 2_000) { refuse "tooSmallToEarn" }
    const earned = checked.claims.amount / 100
    const total  = state.lifetimePoints + earned   // traps on overflow
    const tier   = tierFor(total)
  }

  update  {                                // a patch, not an expression
    lifetimePoints: total
    member.points:  state.member.points + earned
    member.tier:    tier
  }

  execute {                                // the only phase with effects
    credential.issue(LoyaltyMember {
      member_id: next.member.member_id,
      tier:      next.member.tier,
      points:    next.member.points,
    })
  }
}
```

**Four ways an action does not commit**, and they are different on purpose:
`require` fails (a defect — silent), `verify` fails (told plainly), `refuse`
(the application declining, naming a key in the bundle), or the host refuses the
batch.

`update` is a patch: each line names a field and its next value, `:` and not
`=`, and anything unnamed is unchanged. `next` is the state `update` produced.

`execute` **describes** effects and performs none: it emits a request and stops.
The whole batch is offered to the host, which takes all of it or none — and
**the compiler orders what cannot be undone last**.

---

## 7. Effects

```val
credential.issue(Type { … })
payment.request(to: "…", amount: …)
present { disclose … / prove … }
```

`present` is one effect however many lines it has, and an action performs **at
most one disclosure**. `disclose` and `prove` take nothing the module holds: the
host fetches the claim and builds the proof, because handing the claim to the
module would be the same answer with the privacy removed.

`navigate Screen` is written where a press is, not in `execute`.

---

## 8. Screens

Declared, never imperative. The host ships every component; the application says
which, arranged how, bound to what.

```val
@main
screen Portfolio {
  title: phrase("portfolioTitle")

  // Resolved by the host BEFORE anything is drawn — so no half-empty screen and
  // no prompt arriving mid-scroll.
  data {
    holdings: credentials of Holding verified with FromLicensedBroker
      order by market_value desc
      limit 200

    quotes: query broker.quotes(symbols: holdings.symbols) as List<Quote>
  }

  compute {                       // derives; never acts
    const totalValue = holdings.fold(0) { sum, h -> sum + h.claims.market_value }
  }

  column {
    card(text: phrase("total", value: totalValue))
    section(text: "holdings")
    list(holdings) { h ->
      tile(text: phrase("line", symbol: h.claims.symbol))
    }
    button(text: "prove", emphasis: secondary, onTap: ProveAccredited)
  }
}
```

`text:` is **always a key** into the signed bundle, never a sentence — the
compiler checks the key exists, that its slots match, and that every promised
locale has it.

The catalogue (from the host registry, not the language): `accordion audio
avatar badge banner button card carousel checkbox claim column credentialCard
dataTable datePicker divider emptyState field filePicker grid image keyValue
link list notice pick progress qr radioGroup row section select skeleton slider
spinner stat text tile timeline timelineItem toggle video webContent`. A screen
cannot ask for one the host does not draw.

Components are the application's own arrangements of that catalogue — they add
no primitive, and `export component` crosses a package boundary.

---

## 9. Capabilities

Declared once, consented to once, and **checked against use**: one declared and
never used fails the build, and one used and never declared fails it too.

```val
capabilities {
  credential.read(PurchaseReceipt)     // parameterised — the type is part of it
  credential.check(EmployeeBadge)
  credential.issue(LoyaltyMember)
  disclosure.present
  payment.request
  api.query(audience: "broker.co.th", presenting: BrokerageAccount)
}
```

A capability the host registry does not have is an error on the line it is
written.

---

## 10. Expressions

```val
const x = …                      // a definition
let x = …                        // a variable; `x = …` writes it again
a ? b : c                        // `if` is a statement; this is the expression
a ?: b                           // a, unless it is nothing
a?.b                             // nothing, if a is
if (cond) { … } else { … }
switch (x) { A => 1, B => 2 }    // no default over an enum: it must be exhaustive
switch (n) { >= 10000 => …, default => … }   // an arm may be a comparison
{ ...record, field: value }      // derive; never mutate
{ a, b } = record                // after `const` or `let`
0...10                           // both ends included
x with Policy                    // the only way to Verified<P>
x exists                         // narrowing, in `require`
value from { Policy }            // provenance, on an issued claim
```

**Builtins, a closed set:** `duration(days:/hours:/years:)`, `min`, `max`, `abs`.

**List combinators:** `map filter fold any all count first`. There is **no
index** and no `while`: a list is consumed by a combinator, and the bound is
what makes the program total.

```val
receipts.map { r -> r.claims.amount }
receipts.fold(0) { sum, r -> sum + r.claims.amount }
```

---

## 11. Provenance

```val
credential.issue(LoyaltyMember {
  points: next.member.points from { ReceiptFromMerchant }
})
```

`from` is a requirement: this claim may be computed **only** from data verified
under that policy. Mix anything else in and it does not compile. Whoever
receives the credential can check how the number was reached instead of taking a
signature's word for it.

---

## 12. Totality

Every program halts. No unbounded loops, no recursion (the call graph must be
acyclic), no index into a list. Every `fold` has a length bound known at compile
time — which is also what lets `prove` compile to a zero-knowledge circuit, and
what makes a fuel budget an upper bound rather than a guess. Arithmetic traps
rather than wrapping.

---

## 13. What ships, and what a wallet checks

A **`.vapp`** is one signed document: the **compiled module**, the manifest, the
text bundle, the capability report, an integrity hash and a signature.
**No source travels in it** — a wallet has no compiler, so what it is handed is
what runs, and every check it makes is on those bytes.

The wallet checks, itself, on every call:

1. the module hashes to what integrity says
2. the signature is over these bytes, by a key the publisher's DID document publishes
3. it imports **only** what this host provides — an unknown name is refused, not linked
4. the shipped report is the report the module measures to
5. every gate it states is one the module carries the `check` import for
6. every promised locale has every key

That a module is the source somebody published is answered by **reproducible
builds** (`valpack reproduce`), outside the wallet, by whoever cares — not by
shipping source to every phone.

---

## 14. Running it

The host is asked for what the program declared, gets back a **batch of effect
requests**, and decides. What actually ran is an **execution record**: a signed
JWT naming the code hash, the action, the inputs, the state roots before and
after, the batch, and the outcome. A publisher's server verifies that record and
then issues — the application itself has no issuer key and must not have one.

```
device        runs the action, signs the execution record
   ↓
your backend  verifies that record, signs the credential with your issuer key
   ↓
device        stores the credential
```

---

## 15. Known limits, stated plainly

- **`storage.write`, `message.send`, `network.request` do not exist.** They were
  in the registry and no back end emitted them; two of the three argued with
  decisions the language had already made.
- **The operation does not survive into the module.** `query broker.quotes` and
  `query broker.orders` compile to one import — the audience — so a
  module-running host is told who to ask and not what for.
- **`prove` is a declaration today.** The circuit compiler is specified and the
  bounded fragment is enforced; the prover is not written.
- Two back ends exist — a tree-walking evaluator and the Wasm module — and a
  parity test asserts they agree about the whole tree and every effect.

---

## 16. Where to look

| | |
| --- | --- |
| `examples/` | every construct above, compiling and tested |
| `docs/spec.md` · `docs/th/spec.md` | the specification, both languages |
| `docs/guide/` | ten chapters, both languages |
| `crates/valang` | lexer, parser, checks, type checker, printer |
| `crates/valang-wasm` | the Wasm back end and the ABI the report is read from |
| `crates/valang-runtime` | the evaluator, the host trait, execution records |
| `crates/valang-package` | the `.vapp`: build, sign, verify, install |
