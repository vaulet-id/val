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

You cannot understate it. The wallet measures the report from the module's own
import section — what a module can call is what it imports — and refuses the
package if that is not what you shipped.

## Who the application opens for

Most applications open for anybody holding the phone. If yours does not, say so:

```val
capabilities { credential.check(EmployeeBadge) }

admits {
  EmployeeBadge with EmployedByAcme else "notStaff"
}
```

`EmployeeBadge` is declared with the credential type its issuer stamps —
`credential EmployeeBadge as "https://…/credential/employee-badge"` — on
whatever domain that issuer runs. There is no registry to be in.

Without a credential that passes the policy, the application does not draw its
first screen and does not run an action. The person is shown `notStaff` — a key
in your text bundle, so the words are yours, reviewed and translated like every
other sentence you ship.

**The wallet answers the door, and your program never asks.** You cannot write
"does this person hold one?" in VAL, and that is the point: a program that could
ask would be holding the credential in order to find out it was absent. That is
a read, and a gate exists instead of it. Which is why the line above is a
`credential.check` and not a `credential.read` — your application is told it
opened and never what opened it, and the sheet says exactly that.

Both halves are required. Without the policy you admit anything shaped like a
badge, including one somebody made; without the sentence the door closes in
silence, and the person is left with a fault report instead of an instruction.

Next: [credentials and trust](04-credentials-and-trust.md).
