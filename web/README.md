# The VAL playground

An editor, a preview and the capability report, in a browser, with no backend.

**Nothing here executes.** There is no evaluator yet, and a playground that
pretended to run VAL would be teaching a language nobody has shipped. What it
does instead is the part that is already true:

- **the editor** highlights the shell and the expression layer differently,
  because they are read by different people — and marks what would not compile,
  a float where a smaller unit belongs, a capability declared and never used
- **the preview** draws the screen the program declared. The frame is the
  host's, the components are the host's, the interaction state is the host's;
  what the application supplied is the structure and the keys. Slot values are
  shown as the expressions that produced them, because formatting a number for
  a locale is the host's job and never the application's
- **the report** is derived from the code, which is the whole point of it. A
  publisher cannot understate what they never wrote down

It reads the specification and the examples out of the repository rather than
keeping copies of them, so a playground cannot start teaching a language that no
longer exists.

The parser here is written in TypeScript and is not the one in `crates/valang`.
It stops where a preview and a report stop needing it. Where the two disagree,
`docs/spec.md` is right and this is a bug.

```
npm install
npm run dev
```
