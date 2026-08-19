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
    card(text: phrase("balance", points: state.member.points))
    section(text: "history")
    list(receipts) { r ->
      tile(text: phrase("receiptLine", merchant: r.claims.merchant, at: r.claims.purchased_at))
    }
    button(text: "scan", emphasis: primary, onTap: ScanToEarn)
  }
}
```

## One screen opens the application

Mark it `@main`. Every other screen is reached by a press.

```val
@main
screen Wallet { … }

screen Receipt(id: string) { … }
```

A package with more than one screen and no `@main` does not build: which screen
somebody sees first would otherwise depend on the order the files happened to be
read.

## A screen may show one thing or another

```val
@main
screen Wallet {
  column {
    if (state.points > 0) {
      card(text: phrase("balance", points: state.points))
      button(text: "scan", emphasis: primary, onTap: ScanToEarn)
    } else {
      emptyState(text: "notAMember", detail: "joinAtTheCounter")
    }
  }
}
```

`else` is optional. Both branches are checked whichever one runs, and both count
towards what the package declares it does — a capability used only in the branch
nobody took today is still a capability the person consented to.

The wallet never sees the condition. It is settled while the screen is being
resolved, so what arrives to be drawn is one tree with the choice already made.

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
card(text: phrase("key", name: value))
tile(text: phrase("key", name: value), onTap: Action)
list(binding) { item -> … }
button(text: "key", emphasis: primary, onTap: Action)
```

Props are semantic: `text`, `icon`, `emphasis`, `state`, `onTap`. No colours, no
fonts, no pixel sizes. Asking for a component the catalogue does not have is
reported, not approximated.

If you need pixels, use the webview tier and its lower capability ceiling.

## Sharing a component with another package

A component is visible to every file in your package. To let another package
draw it, `export` it — and to draw somebody else's, `import` it by name and
version.

```val
// org.vaulet.ui
export component MoneyCard(label: string, amount: string) {
  card {
    text: label
    Amount(amount: amount)
  }
}

component Amount(amount: string) { text(amount) }
```

```val
// org.vaulet.shop
import "org.vaulet.ui/1" { MoneyCard }

@main
screen Home {
  column {
    MoneyCard(label: "Balance", amount: "120")
  }
}
```

Three things follow from how this is resolved, and they are the parts worth
knowing:

**It happens at build time.** The imported component is expanded where it was
written, then folded into your package. Nothing is linked and nothing is fetched
while somebody is looking at a screen. `Amount` above is private and comes along
without its name, so it cannot collide with an `Amount` of your own.

**What it draws becomes yours to declare.** An imported component that plays a
video needs `media.video` in *your* capabilities block, because the person
consents to one list rather than to one per package involved.

**An exported component takes what it draws as an argument.** It cannot read
`state`, `input` or `context`: those belong to whichever package it lands in, and
a name resolved against the wrong package's state is a mistake neither author can
see.

Text works the same way — a key inside an imported component is looked up in
*your* bundle. You imported the component; you supply the words.

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
