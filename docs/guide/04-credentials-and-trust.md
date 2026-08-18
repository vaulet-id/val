# Credentials and trust

A credential is somebody's signed word about somebody else. Your app says whose
word it will accept, and the language will not let you read one until you have.

## Four faces

```val
receipt.claims        // what the issuer said — your declared fields
receipt.signature     // .valid
receipt.status        // .active
receipt.holder        // .bound — is this the person in front of us
```

The last three are readable only inside `trust` and `verify`.

## Write a policy

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

Bind the subject by name — `holding.signature.valid`, not a bare
`signature.valid`, which stops being unambiguous the moment a second credential
is in scope.

**Put freshness in the policy.** A valuation from last week is signed,
unrevoked, correctly bound — and wrong. In the policy, stale data never reaches
your screen at all.

**Use an anchor, not an issuer.** Pinning a specific issuer puts your allowlist
somewhere the person cannot see, and adding a merchant becomes a new version.

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
one, so forgetting the check does not compile.

**The type names the policy, not the credential.** `Verified<SignatureOnly>` and
`Verified<ReceiptFromMerchant>` are different types. A valid signature says
nothing about revocation or about who issued the credential.

If one policy really does subsume another, declare it:

```val
trust StrictReceipt(r: PurchaseReceipt) refines ReceiptFromMerchant { … }
```

This is checked as containment: your policy must require everything the other
one does, as text.

## Provenance

A number computed from a verified credential remembers that. A number from your
own state, or from an API answer, remembers that too — and they are not the same
fact.

You meet this when you issue something:

```val
credential.issue(LoyaltyMember {
  points: next.member.points from { ReceiptFromMerchant }
})
```

`from` says this claim may only be computed from data verified under that
policy. Mix in anything else and it does not compile. Whoever receives the
credential can then check how the number was reached.

Next: [actions](05-actions.md).
