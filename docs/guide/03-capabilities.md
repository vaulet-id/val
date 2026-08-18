# Capabilities and consent

Everything your application may do is in one block, and a person agreed to that
block before it ran.

```val
capabilities {
  credential.read(PurchaseReceipt)
  credential.issue(LoyaltyMember)
  disclosure.present
  api.query(audience: "broker.co.th", presenting: BrokerageAccount)
}
```

## They name types, not strings

`credential.read(PurchaseReceipt)` is checked. A string would be a second copy of
a name, and the first typo would be found by a customer.

It is also the whole permission: reading `PurchaseReceipt` does not let you read
a passport. Declaring one type and reading another does not compile, and the
error says why — it is not less privilege, it is a different permission wearing
the right label.

## One block, one package

A package can be several files and they share one scope, but the capabilities are
declared once. A person consented to a list, not to the sum of the lists across
your files, and "which file says what this application may do" has to have one
answer.

## Unused is a failure

Declaring something you never use fails the build. This is the rule people push
back on most and it is the one worth keeping: an application that asks for more
than it needs trains the person to stop reading, and the next application is the
one that needed watching.

## What the person actually sees

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

The claims you touch, not just the credentials. Where anything goes. Whether
anything happens that cannot be taken back.

You cannot understate it. The host recomputes the report from your sources and
refuses the package if it does not match what you shipped — because the
interesting adversary is not somebody tampering with your package in transit, it
is a publisher signing a report that flatters their own application.

Which is also why **the sources travel in the package**. A hash over compiled
output proves it is the output somebody signed; it never proves it is the program
somebody read.

Next: [credentials and trust](04-credentials-and-trust.md).
