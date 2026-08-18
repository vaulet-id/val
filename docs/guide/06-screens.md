# Screens

You declare a screen. The host draws it.

```val
screen Wallet {
  data {
    receipts: credentials of PurchaseReceipt verified with ReceiptFromMerchant
      order by purchased_at desc
      limit 50
  }

  column {
    card(text: "balance", points: state.member.points)
    section(text: "history")
    list(receipts) { r ->
      row(text: "receiptLine", merchant: r.claims.merchant, at: r.claims.purchased_at)
    }
    button(text: "scan", emphasis: primary, onTap: ScanToEarn)
  }
}
```

## Declare your data; do not fetch it

The `data` block is resolved by the host **before anything is drawn**. No
half-drawn screen, no permission prompt arriving while somebody is scrolling, and
one block a reviewer can read to learn what this screen sees of a wallet.

`verified with` is doing real work: the list cannot show a receipt that failed
the policy. Not "is filtered out" — cannot appear, because the filtering is the
host's and not a line of your code that somebody could delete.

`limit` is not politeness. It is what makes a total over the list a bounded
computation, and what lets a proof over it compile to a circuit.

## A press names an action

`onTap` names an action you declared. There is no other kind of handler, so
everything a screen can start goes through `require → verify → compute → update →
execute`, with the same consent and the same record. **A screen adds no path to
an effect.**

## The components are the host's

You get what the host ships. On Vaulet today that is a small set — cards, rows,
sections, lists, buttons — and asking for something else does not draw something
approximate, it says the component is not in this catalogue.

Props are **semantic**: `text`, `icon`, `emphasis`, `state`, `onTap`. No colours,
no fonts, no pixel sizes. Every application looks like it belongs to the wallet,
which is a feature for the person and a constraint for you. If you need pixels,
you want the webview tier — and its lower capability ceiling.

Your package records which catalogue version it was built against, and a host
renders those semantics or refuses. A component that quietly means something else
on a later version is a screen the person did not consent to.

## Text is not in your code

`text: "balance"` is a key into your signed bundle, never a sentence:

```json
"balance": { "en": "You have {points} points", "th": "คุณมี {points} แต้ม" }
```

You supply the slots; the host formats them. That is not a division for its own
sake — it means Thai numerals, the Buddhist era and the currency position are
right once for every application, instead of being wrong differently in forty of
them.

Strings cannot be built in VAL at all. No interpolation, no `+`. Every sentence a
person reads comes from something that was signed, which is also what makes the
rule against imitating the wallet's own chrome enforceable.

## Interaction state is not your state

Which tab is open, where a list is scrolled, what is typed in a field that has
not been submitted: all the host's. Your `state` is hashed, signed and replayed —
a scroll position in it would dilute the word "provable" by every press.

You get what the form held **at the moment it was submitted**, through `input`.

## Screens derive but do not act

```val
compute {
  const totalValue = holdings.fold(0) { sum, h -> sum + h.claims.market_value }
}
```

Pure, an action's rules, no effects. Keeping a total in `state` instead would
hash, sign and replay a number that is a function of what is already on the
screen — which is how two copies of one number start disagreeing.

Next: [disclosing and proving](07-disclosing-and-proving.md).
