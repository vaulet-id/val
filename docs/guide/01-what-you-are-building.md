# What you are building

A Micro App runs inside somebody's wallet, beside their passport and their bank
credentials. It reads credentials they already hold, computes, and asks the
wallet to act.

You do not deploy it to a server you control. You publish a signed package, the
person installs it, and it runs on their phone.

## What you get

**No login to build.** The person is already identified by credentials a
government, a bank, a licensed broker or an employer issued. You ask for a claim
and get a claim, already checked.

**No user data to store.** It stays in their wallet. You read it under a policy
you named, use it, and it never becomes a row in your database.

**Proofs without disclosure.** You can have "this person is over twenty"
answered without learning their birthday.

**A signed record of every run.** Each action produces a record: this code, this
version, this state before, this state after. When a customer disputes a
transaction, that is what you have instead of your own log file.

## What you give up

**You do not draw screens.** You declare `card`, `row`, `button`; the wallet
decides how they look. Every app on the platform looks like it belongs there.

**You do not hold credentials.** You get an answer to the question you asked,
not a copy of the document.

**You cannot do anything you did not declare.** Capabilities are checked before
your code runs. Declaring one you never use fails the build.

**Some things are not in the language.** No floating point, no recursion, no
loop whose end is not known, and no concatenating sentences together. See
[the reference](../spec.md).

## VAL or a webview

You can also ship a webview: your own HTML and JavaScript in a frame, talking to
the wallet across a bridge. It is the faster path if you have a web team.

| | webview | VAL |
| --- | --- | --- |
| Draw your own screens | yes | no |
| Read a credential through the wallet's sheet | yes | yes |
| Issue a credential | no | yes |
| Take a payment | no | yes |
| Sign something the person approves | no | yes |

The wallet cannot state what your webview did, so it cannot offer the
capabilities that depend on saying it. If you need those, use VAL.

## What a package contains

```
loyalty.val      the application: capabilities, credentials, actions
wallet.val       a screen, in the same package
text.json        every sentence a person will read, in every language
```

You compile it, sign it, and hand over a `.vapp`. The wallet checks all of it
again from scratch — your build passing proves nothing about your package.

Next: [your first application](02-your-first-application.md).
