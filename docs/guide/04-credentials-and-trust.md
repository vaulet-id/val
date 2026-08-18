# Credentials and trust

A credential is somebody's word about somebody else, signed. Your application's
job is to say **whose word it will accept**, and the language will not let you
read one until you have.

## Four faces

Every credential has the same shape, whatever it carries:

```val
receipt.claims        what the issuer said — your credential's fields
receipt.signature     .valid
receipt.status        .active
receipt.holder        .bound — is this the person in front of us
```

The last three are readable in `trust` and in `verify` and nowhere else. An
application deciding for itself whether a signature is good enough is exactly the
thing a trust policy exists to stop.

## A policy is a named set of conditions

```val
trust FromLicensedBroker(holding: Holding) {
  anchor: "th.go.sec.licensed-brokers"
  require {
    holding.signature.valid
    holding.status.active
    holding.holder.bound
    holding.claims.valued_at > context.time.now - duration(hours: 24)
  }
}
```

The subject is bound by name — `holding.signature.valid`, never a bare
`signature.valid`, which is unambiguous right up until a second credential is in
scope.

That last line is the one worth copying. A valuation from last week is signed,
unrevoked, correctly bound — and wrong. **Freshness is a trust question**, so it
belongs in the policy, and a stale holding then never reaches your screen rather
than reaching it with a warning somebody has to remember to add.

## Anchors, not issuers

`anchor:` names a root that a chain resolves against. Pinning a specific issuer
is possible and is usually a mistake: it puts your own allowlist somewhere the
person cannot see, and adding a merchant becomes a new version of your app.

## Verification is a type

```val
verify {
  const checked = receipt with ReceiptFromMerchant
}

compute {
  const earned = checked.claims.amount / 100
}
```

`checked` is a `Verified<ReceiptFromMerchant>`. There is no cast that produces
one and no way to reach `claims` without it, so the check cannot be forgotten —
forgetting it does not compile.

**The type names the policy, not the credential.** `Verified<SignatureOnly>` and
`Verified<ReceiptFromMerchant>` are different types, and a function that wants
the second will not take the first. A signature that is valid says nothing about
whether the credential was revoked or who issued it, and a type system that
called both of them "verified" would be telling you they were the same fact.

If one policy genuinely subsumes another, say so:

```val
trust StrictReceipt(r: PurchaseReceipt) refines ReceiptFromMerchant { … }
```

Checked as containment — your policy must require everything the other one does,
as text. Not as implication: a checker that decides one predicate implies another
is a checker that is wrong quietly.

## Where values come from travels with them

A number computed from a verified credential remembers that. A number computed
from your own state, or from an API answer, remembers that too — and they are
not the same fact.

You will meet this when you issue something:

```val
credential.issue(LoyaltyMember {
  points: next.member.points from { ReceiptFromMerchant }
})
```

`from` says this claim may only be computed from data verified under that policy.
Mix in anything else and it does not compile. What it buys is that whoever
receives this credential next does not have to take your signature's word for how
the number was reached.

Next: [actions](05-actions.md).
