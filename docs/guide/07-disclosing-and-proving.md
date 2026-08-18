# Disclosing and proving

```val
execute {
  present {
    disclose checked.claims.country
    prove checked.claims.birthdate <= context.time.now - duration(years: 20)
  }
}
```

**`disclose` hands over a value.** They learn the country.

**`prove` hands over an answer.** They learn that somebody is over twenty. They
do not learn the birthday and cannot work it out later.

## Prove is not a comparison you make

Reading the birthday and comparing it is a disclosure with a comparison after
it. You saw the date, and the door's operator is trusting you not to keep it.

`prove` produces a `Proof<bool>` and nothing weaker. Where the wallet cannot
produce a real zero-knowledge proof, **your app does not build**. It never falls
back to disclosing and comparing.

## Both are effects

Disclosure requires `disclosure.present`, appears in `execute` with everything
else, and lands in the execution record.

**One disclosure per action.** The effects in `execute` are one batch, and a
disclosure cannot be undone — a second one could not depend on a batch the first
has already completed. Two disclosures need two consents, which means two
actions.

## What can be proved

`prove` compiles to a circuit, and only part of the language does. The compiler
tells you when you have left it.

**Inside:** integers with a declared width, dates and times compared as
integers, string equality, `switch` and `?:`, and list combinators where the
length is known at compile time — which is what `limit` on a `data` declaration
is for.

Two things to know before you write one:

- **Every branch costs.** A circuit pays for both sides of a conditional. A
  cheap-looking `?:` is two computations.
- **It pays for the bound, not the data.** A proof over a list of at most 200
  costs 200 additions whether the person holds two positions or two hundred.
  That is also why it does not leak how many they hold.

## Proofs over your own state

State is a Merkle tree, so one field can be shown without opening the rest.

```val
disclose state.member.tier
prove state.lifetimePoints >= 10_000
```

Know what you are claiming. A credential claim is backed by an issuer who signed
it. A state field is backed by the chain of records that produced it: correct by
rules anybody can re-run, but with no third party behind the input. The verifier
is told which of the two it is looking at.

## What you cannot prove

Anything that came from an API. A query answer is somebody's word, not somebody's
signature. The compiler refuses it, and the honest alternative is to disclose the
number and say where it came from.

Next: [state and versions](08-state-and-versions.md).
