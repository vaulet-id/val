# Screens

You declare a screen. The wallet draws it.

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

The wallet resolves the `data` block before anything is drawn. No half-drawn
screen, and no permission prompt arriving while somebody is scrolling.

`verified with` means a receipt that fails the policy cannot appear — not "is
filtered out". The filtering is the wallet's, not a line of your code somebody
could delete.

`limit` is required on a list you compute over. It bounds the work, which is
what lets a total over the list compile to a circuit.

## A press names an action

`onTap` names an action you declared. There is no other kind of handler, so
everything a screen can start goes through the six phases with the same consent
and the same record.

## The components are the wallet's

```val
column { … }
section(text: "key")
card(text: "key", slot: value)
row(text: "key", slot: value, onTap: Action)
list(binding) { item -> … }
button(text: "key", emphasis: primary, onTap: Action)
```

Props are semantic: `text`, `icon`, `emphasis`, `state`, `onTap`. No colours, no
fonts, no pixel sizes. Asking for a component the catalogue does not have is
reported, not approximated.

If you need pixels, use the webview tier and its lower capability ceiling.

## Text is not in your code

`text: "balance"` is a key into your signed bundle:

```json
"balance": { "en": "You have {points} points", "th": "คุณมี {points} แต้ม" }
```

You supply the slots; the wallet formats them. Thai numerals, the Buddhist era
and currency position are right once for every app instead of being wrong
differently in forty of them.

## Interaction state is not your state

Which tab is open, where a list is scrolled, what is typed in a field that has
not been submitted: all the wallet's. You get what the form held at the moment
it was submitted, through `input`.

Note: your `state` is hashed, signed and replayed. A scroll position in it would
dilute what "provable" means, one press at a time.

## Screens derive but do not act

```val
compute {
  const totalValue = holdings.fold(0) { sum, h -> sum + h.claims.market_value }
}
```

Pure, an action's rules, no effects. Keep a total here rather than in `state` —
a value you can compute from what is already on the screen does not need to be
stored.

Next: [disclosing and proving](07-disclosing-and-proving.md).
