# Capabilities and consent

Everything your app may do is in one block, and the person agreed to that block
before it ran.

```val
capabilities {
  credential.read(PurchaseReceipt)
  credential.issue(LoyaltyMember)
  disclosure.present
  api.query(audience: "broker.co.th", presenting: BrokerageAccount)
}
```

## Rules

**They name types, not strings.** `credential.read(PurchaseReceipt)` is checked
by the compiler. Declaring one type and reading another does not compile.

**One block per package.** A package may be several files, but capabilities are
declared once. "Which file says what this app may do" has one answer.

**Unused is a failure.** Declaring something you never use fails the build.

Note: this is the rule people push back on. An app that asks for more than it
needs trains the person to stop reading, and the next app is the one worth
watching.

## What the person sees

Not this block. They see the **capability report**, which the compiler derives
from your code:

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

It lists the claims you touch, not just the credentials; where anything goes;
and whether anything happens that cannot be taken back.

You cannot understate it. The wallet recomputes the report from your sources and
refuses the package if it does not match what you shipped.

Note: this is also why the sources travel in the package. A hash over compiled
output proves it is the output somebody signed; it never proves it is the
program somebody read.

Next: [credentials and trust](04-credentials-and-trust.md).
