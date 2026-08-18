# Publishing

```bash
valpack build ./my-app -o my-app.va
valpack verify my-app.va
```

A `.va` is one signed document: your sources, the manifest, the text bundle, the
derived capability report, a hash per file, and a signature over all of it. The
same inputs produce the same bytes, so two people can check they are holding the
same application.

## The sources are in the package

Not the sources *or* compiled output — the sources, always. A hash over a
compiled artifact proves it is the artifact somebody signed; it never proves it
is the program somebody read.

This is what lets the wallet check your package from first principles instead of
trusting your build.

## What the wallet checks before it admits you

1. **Every source hashes to what integrity says.** Nothing was changed after
   signing.
2. **The signature is over these bytes**, by the key your manifest names.
3. **It compiles** — checked there, not taken from a build nobody else ran.
4. **The report it ships is the report its code produces.** Tampering with a
   report breaks the signature, but the case that matters is a publisher signing
   an understatement of their own app. Recomputing catches that.
5. **Every locale your manifest promises has every key.**

Then the wallet's own policy: whether an app of your kind may hold the
capabilities you declared, and whether it can render the catalogue you built
against.

## Who signs the credentials you issue

Not your application. It has no issuer key and must not have one.

```
device        runs the action, signs the execution record
   ↓
your backend  verifies that record, signs the credential with your issuer key
   ↓
device        stores the credential
```

This is why a publisher has a server at all, and it is the whole of what the
server does. It does not run VAL, hold state, or see it. It checks the
signature, resolves the code hash to a version you published, checks the trust
chains, verifies any proof — the verification key comes from the compiled
circuit, so it knows which predicate was proved — and then signs, or refuses.

Somebody holding the device can rewrite their own state. They cannot make your
server sign a credential for a run that does not verify.

## Versions

`version` is on the first line and it is not decoration. A new version is a
fresh consent whenever the capability report differs, the app's kind changes, or
the shape of `state` changes.

The last one takes the state with it. Plan for it: what matters belongs in a
credential, not in `state`.

Next: [the reference](10-reference.md).
