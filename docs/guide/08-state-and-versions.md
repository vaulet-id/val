# State and versions

```val
state {
  member:         LoyaltyMember?
  lifetimePoints: int default 0
}
```

Yours, on their device, changed only by `update` in an action. Use `default`,
never `=` — there is no assignment in this language.

## It is hashed

After every action the wallet builds a Merkle tree over your state's
`(path, value)` leaves and records the root. The next record carries the
previous root, so the two chain.

This is tamper-**evident**, not tamper-proof. The state is on somebody's device
and they can throw it away — this is not a blockchain and nobody else has a
copy. What the chain gives is detection, to anyone who kept an earlier record.

That is why a verifier remembers the last root it saw, and why an issued
credential records the root it came from. Rolling back leaves a signed
credential pointing at a state that no longer exists.

## Keep it small

**No derived values.** A total you can compute from what is already there is a
second copy of a number. Screens have `compute` for this.

**No interaction state.** Which tab is open is the wallet's.

Sizes are bounded by the wallet, and the limit is checked before the state
commits, so a run that would exceed it changes nothing.

## Changing the shape is a new version

**A change to the shape of `state` starts that version's state empty.** There is
no migration, no compatibility shim and no dual reader.

Note: a migration is code that runs against data the current version never
produced, exercised by nobody, and unprovable from an execution record because
no action performed it.

The person is asked again, and what they are asked is legible because there is
no third state that is half of each.

**So do not put anything in `state` you cannot afford to lose.** If it matters,
it belongs in a credential you issued — which the person keeps, and which
survives your next version — or on your own backend.

Next: [publishing](09-publishing.md).
