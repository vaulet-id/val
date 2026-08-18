# Disclosing and proving

Two different things, and the difference is the reason this platform exists.

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
do not learn the birthday, and they cannot work it out later.

## Prove is not a comparison you make

The obvious implementation of an age check is to read the birthday and compare
it. That is a disclosure with a comparison after it — you saw the date, and the
door's operator is trusting you not to keep it.

`prove` produces a `Proof<bool>` and nothing weaker. Where the host cannot
produce a real zero-knowledge proof, **your application does not build**. It does
not fall back to disclosing the birthday and comparing it, because you wrote
`prove` and would never find out that it had not happened.

## Both are effects

Disclosure requires `disclosure.present`, appears in `execute` with everything
else, and lands in the execution record. Handing somebody's data to a third party
is the most consequential thing an application here can do; it is not a footnote.

**One disclosure per action.** The effects in `execute` are one batch, and
nothing un-tells somebody a postcode — a second disclosure could not be
conditional on a batch the first has already completed. Two disclosures want two
consents, which is two actions.

## What can be proved

`prove` compiles to a circuit, and only a fragment of the language does. The
compiler tells you when you have left it, which is the only way the promise above
can be kept — a rule that is discovered at proving time is not a rule.

Inside the fragment: integers with a declared width, dates and times compared as
integers, string equality, `switch` and `?:`, and list combinators where the
length is known at compile time. Which is what the `limit` on a `data`
declaration is for.

Two things worth knowing before you write one:

**Every branch costs.** A circuit pays for both sides of a conditional, not the
one taken. A cheap-looking `?:` is two computations.

**It pays for the bound, not the data.** A proof over a list of at most 200 costs
200 additions whether the person holds two positions or two hundred. That is also
why it does not leak how many they hold.

## Proofs over your own state

You can prove things about `state` too — it is a Merkle tree, so one field can be
shown without opening the rest.

But know what you are claiming. A credential claim is backed by an issuer who
signed it. A state field is backed by the chain of records that produced it: your
application asserted it, correctly, by rules anybody can re-run, and **no third
party stood behind the input**. The verifier is told which of the two they are
looking at, and it is not the same fact.

## What you cannot prove

Anything that came from an API. A query answer is somebody's word, not somebody's
signature, and a proof over it would look exactly as strong as a proof over a
credential while being nothing of the kind. The compiler refuses it, and the
alternative is honest: disclose the number and say where it came from.

Next: [state and versions](08-state-and-versions.md).
