# Actions

An action is the only thing that runs:

```
input → require → verify → compute → update → execute
```

Omit any phase; never reorder them.

## Choosing how an action declines

There are four ways an action does not commit, and picking the wrong one is the
most common mistake in a first app.

| | who sees it | when to use it |
| --- | --- | --- |
| `require` fails | nobody | something that should never be false. If it is, you have a bug |
| `verify` fails | the person | a forged, expired or out-of-anchor credential |
| `refuse "key"` | the person | your own rule: too small, too soon, already claimed |
| the wallet refuses | the person | they said no |

A business rule in `require` gets you a crash where you wanted a message. A bug
in `refuse` gets somebody a polite sentence about a mistake you made.

## Declaring an action does not run it

An action runs because somebody pressed something. If no screen in the package
names it in an `onTap`, nothing can reach it — and the compiler tells you,
because the capabilities it needs are still on the consent sheet.

Actions may live in one file and be pressed from a screen in another; a package
is one scope.

## `input`

```val
input {
  receipt: Credential<PurchaseReceipt>
}
```

Names declared here are in scope for every later phase, bare — `receipt`, not
`input.receipt`.

## `require`

Preconditions, and narrowing an optional:

```val
require {
  state.member exists
  amount > 0
}
```

Use `exists`, not `!= null`. Until you have said it, reading through
`state.member` is a type error rather than a crash later.

## `verify`

Trust policies, and the only place a credential becomes usable. Several
expressions are combined with AND.

## `compute`

Pure. No effects, and no effect can hide behind a function — every `function` in
this language is pure.

Arithmetic traps rather than wraps: overflow and division by zero stop the
action.

Note: a sequence of effects used by three actions has to be written out three
times. In exchange, everything an action can do is in its `execute` block.

## `update`

A patch, not an expression:

```val
update {
  lifetimePoints: total
  member.points:  state.member.points + earned
  member.tier:    tier
}
```

Each line names a field and the value it takes; anything unnamed is unchanged.
Paths may nest but may not contain a list index — build the new list in
`compute` and name it here in one line.

## `execute`

The only place an effect appears, and it does not perform one:

```val
execute {
  credential.issue(LoyaltyMember { points: next.member.points })
}
```

`next` is the state `update` produced. Use it rather than recomputing the same
arithmetic here.

**The effects are one batch.** The wallet takes all of them or none, and your
state commits only if it did. No effect may read another's result — if one
depends on another's outcome, that is two actions.

**At most one disclosure per action.** A disclosure cannot be undone, so a second
one could not depend on a batch the first has already completed.

Irreversible effects are ordered last automatically.

Next: [screens](06-screens.md).
