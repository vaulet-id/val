// Running the handler.
//
// Every language goes to the runner, TypeScript included. Running TypeScript in
// this tab instead would mean the playground executes handlers in a browser
// while production executes them under Node — different globals, different
// fetch, different module resolution — and then claims the two are the same
// thing. The one honest way to show what a handler will do is to run it where it
// will run.

export type Decision =
  | { kind: 'issue'; credential: string; claims: Record<string, unknown> }
  | { kind: 'accept'; note: string }
  | { kind: 'refuse'; refusal: { kind: string; why: string } }
  | { kind: 'threw'; error: string }

/// One server file, keyed by the name the entry point imports it as.
export type ServerFile = { name: string; source: string }

/// The entry point, in whichever language the server is written in.
const ENTRY = /^handler\.(ts|go|rs|py)$/

/// Where the runner is. `cargo run -p valang-runner` locally; set VITE_RUNNER to
/// point the playground at a deployed one.
const RUNNER = import.meta.env.VITE_RUNNER ?? 'http://localhost:8787'

export async function runHandler(
  files: ServerFile[],
  token: string,
  source: string,
  deviceKey: string,
): Promise<Decision> {
  const entry = files.find((f) => ENTRY.test(f.name))
  if (!entry) return { kind: 'threw', error: 'this server has no handler.ts, .go, .rs or .py' }

  try {
    const res = await fetch(`${RUNNER}/v1/run`, {
      method: 'POST',
      headers: { 'content-type': 'application/json' },
      body: JSON.stringify({ files, entry: entry.name, token, source, deviceKey }),
    })
    const body = await res.json()
    if (body.kind === 'busy' || body.kind === 'threw') {
      return { kind: 'threw', error: body.error }
    }
    return body as Decision
  } catch {
    return {
      kind: 'threw',
      error: `no runner at ${RUNNER} — start it with: cargo run -p valang-runner`,
    }
  }
}
