# The VAL playground

An editor, a preview and the capability report, in a browser, with no backend.

**It runs the real compiler.** `valang` and `valang-runtime` are built to Wasm
and loaded into the page, so what a reader is told here is what a host would
say — the same diagnostics, the same derived report, the same evaluator. This
used to be a parser and an evaluator written in TypeScript, honest about being
approximations and still a second implementation of a language whose whole claim
is that what ran can be checked.

- **the editor** highlights the shell and the expression layer differently,
  because they are read by different people — and marks what would not compile,
  from the compiler rather than from an imitation of it
- **the preview** is Flutter — the same toolkit the wallet uses, compiled to web
  and embedded. `button` in a VAL screen means Flutter's button, so a facsimile
  in HTML would have agreed with itself and been wrong about wrapping, about
  Thai line breaking, and about every metric that matters. Slot values appear as
  the expressions that produced them, because formatting a number for a locale
  is the host's job and never the application's. Build it with
  `./preview/build.sh`; it is not checked in, being forty megabytes of CanvasKit
- **the report** is derived from the code, which is the whole point of it. A
  publisher cannot understate what they never wrote down
- **the debugger**, in a pane under the editor where an editor keeps this sort
  of thing. Problems and the log belong together and belong there: both are about
  the program in front of you, and both used to be on the far side of the window
  from it. A problem is clickable and takes you to the line
- **the log** is what a press did. An action is `(state, input, context, code)`
  to `(state\', output, effects)`, which is a reducer — so the panel reads like
  one: what was dispatched, which fields moved, what the host was asked for, and
  the roots before and after. Where it stops resembling a reducer is the part
  worth reading: the effects are requested rather than performed, and the state
  commits only if the host takes them
- **the server is a file too**, under `server` rather than under the package,
  because it runs on the publisher's machine and holds their issuer key — the
  one thing an application must never have. Press Build & Run and one press
  shows the whole transaction: the action on the device, the record it signed,
  and what the publisher's handler did with it. The check the handler runs is
  `valang-verify` compiled to Wasm, which is the same crate a Go or a Python SDK
  will bind to, so this is not a second implementation of the thing it teaches
- **the wallet is a file** — `fixtures/wallet.json`, under `host` rather than
  under the package, because a `.va` never carries somebody\'s wallet. Edit it
  and the screen changes without a line of the application changing, which is
  what "the data is the host\'s" means when it is shown instead of argued

It reads the specification and the examples out of the repository rather than
keeping copies of them, so a playground cannot start teaching a language that no
longer exists.

```
./web/build-wasm.sh     # the compiler and runtime, for the browser
./preview/build.sh      # the renderer, once
npm install && npm run dev
```
