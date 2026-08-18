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

    refuse: (refusal: { kind: string; why: string }): Decision => ({ kind: 'refuse', refusal }),
  }
}

/// Monaco's TypeScript worker, used as a transpiler. It is the same service that
/// type-checks the file in front of you, so what runs is what was checked.
async function transpile(monaco: typeof Monaco, code: string): Promise<string> {
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

  const uri = monaco.Uri.parse('inmemory://server/handler.ts')
  const model = monaco.editor.getModel(uri) ?? monaco.editor.createModel(code, 'typescript', uri)
  model.setValue(code)

  const worker = await monaco.languages.typescript.getTypeScriptWorker()
  const client = await worker(uri)
  const output = await client.getEmitOutput(uri.toString())
  return output.outputFiles[0]?.text ?? ''
}

export async function runHandler(
  monaco: typeof Monaco,
  code: string,
  token: string,
  source: string,
  deviceKey: string,
): Promise<Decision> {
  try {
    const js = await transpile(monaco, code)
    // The module's default export, without a module loader: the emitted code
    // assigns to `exports.default`, so give it an `exports` to assign to.
    const exports: { default?: (t: string, v: unknown) => Promise<Decision> } = {}
    // eslint-disable-next-line no-new-func
    const load = new Function('exports', 'module', `${js}\nreturn exports`)
    const loaded = load(exports, { exports }) as typeof exports
    const handler = loaded.default ?? exports.default
    if (!handler) return { kind: 'threw', error: 'the handler has no default export' }

    const decision = await handler(token, sdk(source, deviceKey))
    return decision ?? { kind: 'threw', error: 'the handler returned nothing' }
  } catch (e) {
    return { kind: 'threw', error: e instanceof Error ? e.message : String(e) }
  }
}
