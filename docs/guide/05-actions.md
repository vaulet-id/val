# Actions

An action is the only thing that runs. It is a function:

```
(previous state, input, runtime context, code) → (new state, output, effects)
```

Everything else in the language exists so that signature can be true — which is
what makes a run replayable, and a replayable run is the only kind that can be
proved.

```
input → require → verify → compute → update → execute
```

Omit any of them; never reorder them.

## The four ways an action does not commit

They are different things to different people, and choosing the wrong one is the
most common mistake in a first application.

| | who sees it | when to use it |
| --- | --- | --- |
| `require` fails | nobody | a thing that should never be false. It being false is your bug |
| `verify` fails | the person | the credential was forged, stale, or outside your anchor |
| `refuse "key"` | the person | your own rule: too small, too soon, already claimed |
| the host refuses | the person | they said no |

A business rule in `require` gets you a crash where you wanted a message. A bug
in `refuse` gets somebody a polite sentence about a mistake you made.

## Declaring one binds nothing

An action happens because somebody pressed something. If no screen in the package
names it in an `onTap`, nothing can reach it — and the compiler says so, because
**the capabilities it needs are still on the consent sheet a person agreed to**.

Actions may live in one file and be pressed from a screen in another: a package
is one scope. A package with no screens at all is a fragment waiting for the rest
of itself, and is left alone.

## `input`

```val
input {
  receipt: Credential<PurchaseReceipt>
}
```

Names declared here are in scope for every later phase, bare — `receipt`, not
`input.receipt`, the way a function's parameters are.

## `require`

Preconditions, and narrowing:

```val
require {
  state.member exists
  amount > 0
}
```

`exists` rather than `!= null`, because the people who most need to read this
block are the ones for whom `null` is jargon. Until you have said it, reading
through `state.member` is a type error rather than a crash later.

## `verify`

Trust policies, and the only place a credential becomes usable. Multiple
expressions are implicitly ANDed.

## `compute`

Pure. No effects, and **no effect can hide behind a function** — every `function`
in this language is pure, so there is no effectful helper to call.

That costs you something real: a sequence of effects used by three actions gets
written out three times. In exchange, "what can this action do" is one block you
can read, rather than a call graph you have to follow. For a language whose whole
purpose is that question, the trade is not close.

Arithmetic here traps rather than wraps. Overflow and division by zero stop the
action, because a wrong number that the execution record would then faithfully
prove is worse than a failure.

## `update`

A patch, not an expression:

```val
update {
  lifetimePoints: total
  member.points:  state.member.points + earned
  member.tier:    tier
}
```

Each line names a field and the value it takes; anything unnamed is unchanged. A
colon and not `=` — there is no assignment anywhere in this language — and the
previous state is still readable as `state.…` on the right of every line.

Paths may nest and may not contain a list index. If you need a new list, build it
in `compute` and name it here in one line.

## `execute`

The only place an effect appears, and it does not perform one:

```val
execute {
  credential.issue(LoyaltyMember { points: next.member.points })
}
```

`next` is the state `update` produced. Recomputing the same arithmetic here
instead is how two copies of one number start disagreeing.

**The effects here are one batch.** The host takes all of them or none, and your
state commits only if it took them. No effect may read another's result — if one
genuinely depends on another's outcome, that is two actions, and the person gets
to see both.

Irreversible effects are ordered last automatically, and an action performs **at
most one disclosure**: nothing un-tells somebody a postcode, so a second one
could not be conditional on a batch the first has already completed.

Next: [screens](06-screens.md).
