# Publishing

```bash
valpack build ./my-app -o my-app.vapp
valpack verify my-app.vapp
```

A `.vapp` is one signed document: **the compiled module**, the manifest, the
text bundle, the capability report, an integrity hash, and a signature over all
of it. The same sources produce the same bytes, so two people can check they are
holding the same application.

## The sources are not in the package

**You compile; the wallet never does.** A phone with a compiler in it is a phone
that has to be handed a program, and the thing it would then run is one nobody
outside your build ever saw. So what ships is the module, and the wallet's
checks are made against *that* — the artifact it is actually going to run.

This is not the weaker position it sounds like. The report is not read off your
manifest; it is **measured from the module's own import section**, which is the
list of what the code can call. A module calls what it imports and nothing else,
so an understated report is not a lie a publisher can tell — it is a module that
would trap the moment it reached for what it did not declare.

What the sources are for is `valpack reproduce`: anybody with your published
source builds it and compares the bytes. That ties module to source *outside*
the wallet, which is where a check that needs a compiler belongs.

## What the wallet checks before it admits you

1. **The module hashes to what integrity says.** Nothing was changed after
   signing.
2. **The signature is over these bytes**, by the key your manifest names — and
   for a `did:web`, that the key is one that name publishes.
3. **The report is the report the module measures to.** Not the one you wrote
   down: it is recomputed from the imports and compared.
4. **It imports nothing this host does not provide.** An unknown import is
   refused at install rather than at the press that would have reached it.
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
