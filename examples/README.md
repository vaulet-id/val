# Examples

**These are proposals, not the specification.** `docs/spec.md` pins the shape —
the phases, the purity rule, `Verified<T>`, no floats — and leaves most of the
concrete syntax open. Everything below fills those gaps in, so that the gaps are
arguable instead of theoretical: the first thing a parser does is force a
hundred decisions the prose never had to take.

Where an example invents something the spec does not settle, it says so in a
comment. Those comments are the to-do list, and they are empty at the moment:
everything the examples reached for has been decided one way or the other.

They are meant to become test cases. An example that stops parsing is a change
somebody made; that is the point of keeping them here rather than in a document.

| | |
| --- | --- |
| [`loyalty.val`](loyalty.val) | the whole action lifecycle — a receipt earns points and reissues a membership |
| [`door.val`](door.val) | the small end: prove an age, disclose nothing |
| [`wallet.val`](wallet.val) | a screen — declared data, host-owned interaction state, a press that names an action |
| [`portfolio.val`](portfolio.val) | an investment portfolio — no state, no issuance, fractional shares without floats, and a proof that discloses nothing |
| [`rejected.val`](rejected.val) | programs that must not compile, and the error each one is owed |
