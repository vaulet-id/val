# State and versions

```val
state {
  member:         LoyaltyMember?
  lifetimePoints: int default 0
}
```

Yours, on their device, changed only by `update` in an action. `default`, never
`=` — there is no assignment in this language and one exception in the
declaration of persistent state would be the worst place to keep it.

## It is hashed, and that is the point

After every action the host builds a Merkle tree over your state's
`(path, value)` leaves and records the root. The record of the next action
carries the previous root, so the two chain: rewriting a state without re-running
the actions that produced it breaks the chain the moment the next one runs.

That is not tamper-*proof*. The state is on somebody's device and they can throw
the whole thing away — this is not a blockchain, nobody else has a copy, and
pretending otherwise would be the wrong promise. It is tamper-*evident*, to
anybody who kept an earlier record.

Which is why a verifier remembers the last root it saw from a person, and why an
issued credential records the root it was derived from. Rolling back then leaves
a signed credential pointing at a state that no longer exists.

## Keep it small, and keep it yours

Two things follow from the state being hashed:

**Do not put derived values in it.** A total you can compute from what is already
there is a second copy of a number, and the copies disagree the day somebody
edits one. Screens have `compute` for exactly this.

**Do not put interaction state in it.** Which tab is open is the host's. In
`state` it would be signed and replayed, and "provable" would mean less by one
press each time.

Sizes are bounded by the host — totality guarantees your program halts and says
nothing about how large a value gets. A `fold` whose accumulator grows is finite
in steps and unbounded in bytes. The limit is checked before the state commits,
so a run that would exceed it changes nothing.

## Changing the shape is a new version

**A change to the shape of `state` is a new version, and its state starts
empty.** No migration, no compatibility shim, no dual reader.

That is a stronger rule than most platforms have, and the reason is specific: a
migration is code that runs against data the current version never produced,
exercised by nobody, and **unprovable from an execution record because no action
performed it**. In a system whose entire claim is that what ran can be proved, an
unprovable step at the boundary between versions is not a small exception.

The person is asked again, and what they are asked is legible precisely because
there is no third state that is half of each.

**So do not put anything in `state` you cannot afford to lose.** If it matters,
it belongs in a credential you issued — which the person keeps, and which
survives your next version — or on your own backend.

Next: [publishing](09-publishing.md).
