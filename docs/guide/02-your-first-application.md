# Your first application

A loyalty card. Somebody scans a receipt, earns a point per baht, and their
membership credential is reissued at the new balance.

It is a small application and it uses everything: reading a credential, checking
who issued it, computing, changing state, and asking the host to issue something
new. Build this and the rest of the guide is detail.

## Say who you are and what you may do

```val
app "th.co.codefin.loyalty"
version 1

capabilities {
  credential.read(PurchaseReceipt)
  credential.issue(LoyaltyMember)
}
```

Two things a person will see before they install this, and the only two things
this application will ever be allowed to do.

Declare a capability you do not use and the build fails. That is not tidiness:
consent asked for something unused is consent spent on nothing, and it is how
people learn to press yes without reading.

## Describe the data

```val
enum Tier { bronze, silver, gold }

credential PurchaseReceipt {
  merchant:     string
  amount:       int        // satang
  purchased_at: datetime
}

credential LoyaltyMember {
  member_id: string
  tier:      Tier
  points:    int
}

state {
  member:         LoyaltyMember?
  lifetimePoints: int default 0
}
```

`amount` is in satang because **there is no floating point**. Money in this
language is always minor units — and so is everything else that would want a
decimal point. A quantity of shares is micro-shares; a percentage is basis
points.

`state` is yours and it persists between actions. `member` is optional, written
`LoyaltyMember?`, which means you cannot read through it until you have said it
exists.

## Say who you trust

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

A trust policy is the only way to get at what a credential says. It names an
**anchor** — a root the chain resolves against — rather than a specific issuer,
so a merchant joining your scheme does not mean shipping a new version of your
app.

The three faces above are on every credential: was it signed, has it been
revoked, is it held by the person in front of you. An application that decided
those for itself is the thing trust policies exist to prevent.

## Write the action

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

    const satangPerBaht = 100
    const earned = checked.claims.amount / satangPerBaht
    const total  = state.lifetimePoints + earned
    const tier   = tierFor(total)
  }

  update {
    lifetimePoints: total
    member.points:  state.member.points + earned
    member.tier:    tier
  }

  execute {
    credential.issue(LoyaltyMember {
      member_id: next.member.member_id,
      tier:      next.member.tier,
      points:    next.member.points,
    })
  }
}
```

Six phases, in that order, and you may omit any of them.

**`require`** is for things that should never be false. If one is, it is a bug in
your application: the action stops and nobody is shown a message.

**`verify`** is where a credential becomes usable. `receipt with
ReceiptFromMerchant` produces a `Verified<ReceiptFromMerchant>` — and that type,
naming the policy, is the only way to reach `claims`. Failing here is ordinary:
a forged or stale receipt, and the person is told.

**`compute`** is pure. No effects, and `refuse` is how you decline for your own
reasons — naming a key in your text bundle, because the sentence somebody reads
has to be one that was signed.

**`update`** describes the next state as a table of what changed. Not
assignment: a colon, because the line says what the next state is while the
previous one is still readable on the right of it.

**`execute`** is the only place an effect appears — and it does not perform one.
It builds a request, the host asks the person, and your state commits only if
they said yes.

## Add the sentences

Nothing in the code above is a sentence somebody reads. Those live in
`text.json`, one entry per key, one line per language:

```json
{
  "locales": ["en", "th"],
  "keys": {
    "tooSmallToEarn": {
      "en": "Purchases under ฿20 do not earn points",
      "th": "ยอดต่ำกว่า 20 บาท ยังไม่ได้แต้ม"
    }
  }
}
```

A key with no translation for a locale you ship to is a **failed build**, not a
bug report. So is a key your code names and the bundle does not have.

## Check it

```bash
valc loyalty.val        # reads text.json beside it
```

Diagnostics first, then the capability report — which is what a person will be
shown, derived from your code rather than written by you:

```
th.co.codefin.loyalty v1
reads          PurchaseReceipt.amount, PurchaseReceipt.purchased_at
               under ReceiptFromMerchant
discloses      —
issues         LoyaltyMember
writes state   lifetimePoints, member.points, member.tier
irreversible   none
```

Read that as your user would. If it says something you did not intend, the code
says it — the report cannot be edited, and the host recomputes it anyway.

Next: [capabilities and consent](03-capabilities.md).
