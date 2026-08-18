// Running the handler.
//
// Transpiled by Monaco's own TypeScript worker — already loaded, so no second
// toolchain — and then executed here, in the reader's tab, with the SDK
// injected. A real host runs this isolated; the plan for that is Firecracker,
// and it is not what an editor needs.

import type * as Monaco from 'monaco-editor'
import * as val from './wasm'

export type Decision =
  | { kind: 'issue'; credential: string; claims: Record<string, unknown> }
  | { kind: 'accept'; note: string }
  /// A handler in a language this tab cannot execute. The record was produced
  /// and signed either way — what is missing is somewhere to run the handler.
  | { kind: 'runner'; language: string; entry: string }
  | { kind: 'refuse'; refusal: { kind: string; why: string } }
  | { kind: 'threw'; error: string }

type Verified = Awaited<ReturnType<typeof verifyToken>>

function verifyToken(token: string, source: string, deviceKey: string) {
  return val.verifyRecord(token, source, deviceKey)
}

/// The SDK a publisher's handler is handed. Small on purpose: everything here
/// is either a call into the verifier or a way of saying what was decided.
function sdk(source: string, deviceKey: string) {
  return {
    verify: (token: string) => verifyToken(token, source, deviceKey),

    issuance(checked: Verified, credential: string) {
      if (!checked.ok) return null
      const effect = checked.effects?.find(
        (e) => e.capability === 'credential.issue' && (e.payload as { credential?: string })?.credential === credential,
      )
      return (effect?.payload as { claims?: Record<string, unknown> })?.claims ?? null
    },

    issue: (credential: string, claims: Record<string, unknown>): Decision => ({
      kind: 'issue',
      credential,
      claims,
    }),

    /// A run that earns no credential but is not an error. Without this, a
    /// handler with nothing to issue could only refuse, and every ordinary
    /// press read as a failure.
    accept: (note: string): Decision => ({ kind: 'accept', note }),

    refuse: (refusal: { kind: string; why: string }): Decision => ({ kind: 'refuse', refusal }),
  }
}

/// Monaco's TypeScript worker, used as a transpiler. It is the same service that
/// type-checks the file in front of you, so what runs is what was checked.
async function transpile(monaco: typeof Monaco, name: string, code: string): Promise<string> {
  // CommonJS, because what runs it is a `new Function` with an `exports` to
  // assign to rather than a module loader. Emitting ESM produced `export
  // default` in a place nothing could import from, which threw at the first
  // token and said nothing about why.
  monaco.languages.typescript.typescriptDefaults.setCompilerOptions({
    target: monaco.languages.typescript.ScriptTarget.ES2020,
    module: monaco.languages.typescript.ModuleKind.CommonJS,
    allowJs: true,
    noEmit: false,
  })

  const uri = monaco.Uri.parse(`inmemory://server/${name}`)
  const model = monaco.editor.getModel(uri) ?? monaco.editor.createModel(code, 'typescript', uri)
  model.setValue(code)

  const worker = await monaco.languages.typescript.getTypeScriptWorker()
  const client = await worker(uri)
  const output = await client.getEmitOutput(uri.toString())
  return output.outputFiles[0]?.text ?? ''
}

/// One server file, keyed by the name `handler.ts` would import it as.
export type ServerFile = { name: string; source: string }

/// The entry point, in whichever language the server is written in. A server
/// has one: `handler.ts`, `handler.go`, `handler.rs` or `handler.py`.
const ENTRY = /^handler\.(ts|go|rs|py)$/

/// Where a language runs.
///
/// TypeScript is transpiled and executed in this tab. The other three are
/// compiled languages or need an interpreter, so they run in the sandbox the
/// hosted runner provides — the same one that will run them in production.
const IN_BROWSER = 'ts'

/// Where the other three run. A local runner by default; set VITE_RUNNER to
/// point the playground at a deployed one.
const RUNNER = import.meta.env.VITE_RUNNER ?? 'http://localhost:8787'

function entryOf(files: ServerFile[]): ServerFile | undefined {
  return files.find((f) => ENTRY.test(f.name))
}

function languageOf(name: string): string {
  return name.split('.').pop() ?? 'ts'
}

/// A module id as written in an import, resolved to a file name: `./sign`,
/// `./sign.ts` and `sign.ts` are the same file.
function resolve(id: string, files: ServerFile[]): ServerFile | undefined {
  const bare = id.replace(/^\.\//, '').replace(/\.ts$/, '')
  return files.find((f) => f.name.replace(/\.ts$/, '') === bare)
}

/// Load a module and everything it imports, once each.
///
/// A cache rather than a fresh evaluation per import, so a module holding
/// state holds one lot of it — and so a cycle stops instead of recursing until
/// the tab does.
function loader(compiled: Map<string, string>, files: ServerFile[]) {
  const loaded = new Map<string, Record<string, unknown>>()

  const load = (name: string): Record<string, unknown> => {
    const already = loaded.get(name)
    if (already) return already

    const exports: Record<string, unknown> = {}
    loaded.set(name, exports)

    const js = compiled.get(name) ?? ''
    const require = (id: string) => {
      const target = resolve(id, files)
      if (!target) throw new Error(`no file named ${id} in this server`)
      return load(target.name)
    }
    // eslint-disable-next-line no-new-func
    new Function('exports', 'module', 'require', js)(exports, { exports }, require)
    return exports
  }

  return load
}

/// Hand the whole server to the runner and let it choose the toolchain.
///
/// The runner verifies the record itself, in Rust, and the SDK it injects
/// returns that result — so a handler in Go and this tab's TypeScript cannot
/// disagree about whether a record was good.
async function onRunner(
  files: ServerFile[],
  entry: string,
  token: string,
  source: string,
  deviceKey: string,
): Promise<Decision> {
  try {
    const res = await fetch(`${RUNNER}/v1/run`, {
      method: 'POST',
      headers: { 'content-type': 'application/json' },
      body: JSON.stringify({ files, entry, token, source, deviceKey }),
    })
    const body = await res.json()
    if (body.kind === 'threw') return { kind: 'threw', error: body.error }
    return body as Decision
  } catch {
    return {
      kind: 'threw',
      error: `no runner at ${RUNNER} — start it with: cargo run -p valang-runner`,
    }
  }
}

export async function runHandler(
  monaco: typeof Monaco,
  files: ServerFile[],
  token: string,
  source: string,
  deviceKey: string,
): Promise<Decision> {
  try {
    const entry = entryOf(files)
    if (!entry) {
      return { kind: 'threw', error: 'this server has no handler.ts, .go, .rs or .py' }
    }

    const language = languageOf(entry.name)
    if (language !== IN_BROWSER) {
      return await onRunner(files, entry.name, token, source, deviceKey)
    }

    const compiled = new Map<string, string>()
    for (const f of files) {
      if (languageOf(f.name) === IN_BROWSER) compiled.set(f.name, await transpile(monaco, f.name, f.source))
    }

    const exports = loader(compiled, files)(entry.name)
    const handler = exports.default as ((t: string, v: unknown) => Promise<Decision>) | undefined
    if (!handler) return { kind: 'threw', error: `${entry.name} has no default export` }

    const decision = await handler(token, sdk(source, deviceKey))
    return decision ?? { kind: 'threw', error: 'the handler returned nothing' }
  } catch (e) {
    return { kind: 'threw', error: e instanceof Error ? e.message : String(e) }
  }
}
