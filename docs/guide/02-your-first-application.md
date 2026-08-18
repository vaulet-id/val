# Your first application

You will build a loyalty card. Somebody scans a receipt, earns a point per baht,
and their membership credential is reissued at the new balance.

It is small and it uses everything: reading a credential, checking who issued
it, computing, changing state, and asking the wallet to issue something new.

Open the playground and follow along, or create `loyalty.val` and `text.json` in
a directory.

## 1. Declare what the app is and what it may do

```val
app "th.co.codefin.loyalty"
version 1

capabilities {
  credential.read(PurchaseReceipt)
  credential.issue(LoyaltyMember)
}
```

These are the only two things this app will ever be allowed to do, and the
person sees both before installing.

Caution: declaring a capability you do not use fails the build. Do not add
capabilities in advance of needing them.

## 2. Describe the data

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

`amount` is in satang because there is no floating point. Money is always minor
units; a percentage is basis points.

`state` persists between actions. `member` is optional — `LoyaltyMember?` — so
you cannot read through it until you have said it exists.

## 3. Say who you trust

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

A trust policy is the only way to reach what a credential says. `anchor` names a
root the chain resolves against, so a merchant joining your scheme does not mean
shipping a new version.

The three checks above are on every credential: was it signed, has it been
revoked, is it held by the person in front of you.

## 4. Write the action

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

Six phases, always in this order. You may omit any of them.

| phase | what it is for |
| --- | --- |
| `require` | things that should never be false. If one is, you have a bug and nobody is shown a message |
| `verify` | where a credential becomes usable. Failing here is ordinary, and the person is told |
| `compute` | pure calculation. `refuse` declines for your own reasons, naming a key in `text.json` |
| `update` | the next state, as a table of what changed. `:` not `=` |
| `execute` | effects. It builds a request; the wallet asks the person, and your state commits only if they agree |

`receipt with ReceiptFromMerchant` produces a `Verified<ReceiptFromMerchant>`.
That type is the only way to reach `.claims`, so the check cannot be forgotten.

## 5. Add the sentences

No sentence a person reads is in the code. They live in `text.json`:

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

A key your code names that the bundle does not have is a failed build. So is a
key with no translation for a locale you ship to.

## 6. Check it

```bash
valc loyalty.val
```

You get diagnostics first, then the capability report — what the person will be
shown, derived from your code:

```
th.co.codefin.loyalty v1
reads          PurchaseReceipt.amount, PurchaseReceipt.purchased_at
               under ReceiptFromMerchant
discloses      —
issues         LoyaltyMember
writes state   lifetimePoints, member.points, member.tier
irreversible   none
```

Read it as your user would. If it says something you did not intend, your code
says it — the report cannot be edited, and the wallet recomputes it anyway.

Next: [capabilities and consent](03-capabilities.md).
