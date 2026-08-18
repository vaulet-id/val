# What you are building

A Micro App runs inside somebody's wallet, next to their passport and their bank
credentials. That is the whole of why this language is shaped the way it is.

You are not shipping a program that runs on a server you control. You are asking
a stranger to let your code sit beside the most sensitive things they own — and
the answer to "why should I" cannot be "trust us". So the platform is built so
that they do not have to: what your application may do is derived from your code
and shown to them before they install it, and what it did is recorded in a way
they can hand to somebody else.

That costs you things. It is worth knowing which ones before you start.

## What you get

**You never build a login.** The person is already identified, by credentials
somebody who matters issued — a government, a bank, a licensed broker, an
employer. You ask for a claim and you get a claim, checked.

**You never store their data.** It stays in their wallet. You read it under a
policy you named, use it, and it does not become a row in your database that you
are then responsible for.

**You can prove things without seeing them.** "This person is over twenty" is a
question you can have answered without learning their birthday. Most systems
cannot do this, and most systems ask for the birthday.

**What you did is provable.** An action produces a signed record: this code, this
version, this state before, this state after. When a customer disputes a
transaction, that is what you have instead of a log file you wrote yourself.

## What you give up

**You do not draw the screen.** The host does. You declare `card`, `row`,
`button`; it decides what those look like. Every application on the platform
looks like it belongs to the platform, which is a feature for the person and a
constraint for you.

**You do not hold their credentials.** You get an answer to a question you asked,
not a copy of the document.

**You cannot do anything you did not declare.** Capabilities are checked before
your code runs, and a capability you never use is a build failure — asking for
something you do not need is how consent stops meaning anything.

**Some things are simply not in the language.** No floating point. No loops. No
recursion. No way to build a string. Each of those has a reason, and the reasons
are in [the specification](../spec.md) if you want them; the practical version is
that a Micro App is a small, exact thing, and the language will not let you make
it a large, approximate one.

## Two kinds of Micro App

You can also write a webview: your own HTML and JavaScript, in a frame, talking
to the wallet across a thin bridge.

It is a real option and it is the faster way to bring an existing web team.
What it cannot do is anything whose safety depends on the host knowing what ran
— because a webview runs code the host did not compile and draws screens it did
not draw.

| | webview | VAL |
| --- | --- | --- |
| Draw your own screens | yes | no |
| Read a credential through the host's sheet | yes | yes |
| Issue a credential | no | yes |
| Take a payment | no | yes |
| Sign something the person approves | no | yes |

That is not a policy we chose to pressure you with. It is a consequence: the
host cannot state what your webview did, so it cannot offer you the capabilities
that depend on saying it. If you need those, you need VAL.

## What a Micro App is made of

A package — one directory, several files, one identity:

```
loyalty.val      the application: capabilities, credentials, actions
wallet.val       a screen, in the same package
text.json        every sentence a person will read, in every language
```

You compile it, sign it, and hand over a `.va`. The host checks all of it again
from scratch, because your build passing proves nothing about your package —
it is your build.

Next: [your first application](02-your-first-application.md).
