# Why a new language

The first question anybody sensible asks: there are plenty of languages, so why
another one.

Three things would have been easier, and each is better than VAL at exactly one
thing.

## An SDK in a language you already know

Nothing to learn, every editor understands it, and the ecosystem is already
there. It is the right answer for most problems.

It is the wrong answer here because a function call is invisible until it runs.
The wallet cannot say what an app will ask for before it asks, so the person
cannot be told either — and the screen that asks for their consent ends up
written by the app doing the asking.

A declaration can be read before anything happens. It can be diffed against the
version somebody already trusted, signed by whoever published it, and refused.

## A webview and a bridge

Fastest of all to adopt, because the team already has a web app and can put it
in a frame with a few functions bolted to the side.

But the wallet did not compile that code and did not draw those pixels. It
cannot state what ran, and it cannot state what the person saw. Every capability
whose safety rests on one of those two — issuing a credential, taking a payment,
signing something the app composed — has to be withheld.

## A manifest plus arbitrary code

Declare the capabilities up front, then write the logic in whatever you like.
This gets you most of the security, and it is a real design: it is what a
webview app on a host like Vaulet is.

What it does not get you is a decision anybody can reproduce. *Why was this
approved* has an answer only when the logic can be analysed and the run is
deterministic — and "arbitrary code" is precisely the part that is neither.

## Why not a restricted subset of TypeScript?

The strongest version of the objection, and the one worth answering properly:
take a language everybody knows, forbid the dangerous parts, ship a linter.

**A subset is a promise you cannot keep.** Anything not explicitly forbidden
stays reachable, and the tooling around the language — the editor, the type
checker, the package manager, the thousand transitive dependencies — does not
know your rules and was not built to enforce them. "No effects in this block",
inside a language where effects are everywhere, is a code review rather than a
compiler.

And the parts that make VAL worth having are not subtractions:

- `Verified<T>` has to be a type the checker enforces, which means owning the
  checker anyway.
- Determinism means no clock, no randomness, no floating point and no network
  reachable at all. By then the subset is a different language wearing
  TypeScript's syntax — with all of TypeScript's surface still to audit.
- The wallet renders the screens, so the interface cannot be an arbitrary
  component tree in the first place.

What survives all of that is small enough to specify in one document. That
matters more than it sounds: the specification is what somebody reads when they
are deciding whether to trust an app written in it.

## When VAL is the wrong answer

If an application needs arbitrary computation, an interface of its own design,
or a library ecosystem, VAL will fight it at every turn — and it should be an
ordinary application that asks a wallet for proofs over OpenID4VP instead.

VAL is for the small, credential-shaped things where somebody will later have to
explain the outcome: which credential, which policy, which rule, which run.

Next: [what you are building](01-what-you-are-building.md).
