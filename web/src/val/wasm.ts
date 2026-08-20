import core from '../../../hosts/core.json?raw'
import vaulet from '../../../hosts/vaulet.json?raw'

/// What this host provides — everything it draws and everything it does, in one
/// registry. A wallet hands its own to the compiler; the language carries no
/// list of what anybody can do, so the playground supplies Vaulet's the way a
/// real host would.
export const HOSTS = [core, vaulet]

// The compiler and the runtime, loaded into the page.
//
// Two exported functions and a length-prefixed string. No binding generator:
// that would be a build step and a version to keep matched, for an interface
// that fits on this page.

export type Diagnostic = {
  line: number
  column: number
  /// How far the squiggle runs, in characters. The editor used to draw eight
  /// of them and underline whatever was there.
  length: number
  severity: 'error' | 'warning'
  message: string
}

export type Report = {
  app: string
  version: string
  reads: string[]
  discloses: string[]
  proves: string[]
  issues: string[]
  audiences: string[]
  payments: string[]
  writes: string[]
  irreversible: boolean
}

export type UiNode = {
  kind: string
  args: Record<string, string>
  lambda: string | null
  children: UiNode[]
}

/// A screen resolved against the wallet: values in the slots rather than the
/// expressions that produced them, and the rows a `list` actually has. The
/// renderer draws and formats; it does not resolve, because resolving is where
/// `limit`, `order by` and `verified with` live.
export type Resolved = {
  name: string
  data: { name: string; grade: string; of: string; policy: string | null; rows: number }[]
  derived: Record<string, unknown>
  tree: { kind: string; args: Record<string, unknown>; children: unknown[] }[]
}

export type Screen = {
  name: string
  data: { name: string; source: string; type?: string; policy?: string; audience?: string }[]
  tree: UiNode[]
}

export type Analysis = {
  diagnostics: Diagnostic[]
  report: Report
  screens: Screen[]
  actions: string[]
  /// What this program declares. An editor offers the names that exist here
  /// rather than every word it has seen in the file.
  names: Declared
}

export type Declared = {
  state: string[]
  credentials: string[]
  types: string[]
  trusts: string[]
  functions: string[]
  components: string[]
  screens: string[]
  enums: { name: string; members: string[] }[]
}

/// The language's own words, from the compiler rather than from a copy.
///
/// A list of keywords written into the editor is a list that drifts: the day
/// `let` was added the editor did not know it, and the day one is removed it
/// will still offer it.
export type Words = { keywords: string[]; phases: string[]; effects: string[] }

let cachedWords: Words | null = null

export function words(): Words {
  if (!cachedWords) cachedWords = call('val_words', {}) as Words
  return cachedWords
}

/// The blocks a position is inside, outermost first — `screen Home`, then the
/// `column` in it, then the `button` in that.
export type Block = { kind: BlockKind; name: string }

export type BlockKind =
  | 'capabilities'
  | 'enum'
  | 'fields'
  | 'trust'
  | 'require'
  | 'function'
  | 'action'
  | 'phase'
  | 'screen'
  | 'data'
  | 'compute'
  | 'component'
  | 'tree'
  | 'node'
  | 'statements'
  | 'switch'
  | 'record'

/// Where the cursor is, according to the parser.
///
/// Line and column are one-based, the way the editor counts and the way a
/// diagnostic reports. A file that is still being typed answers too: a block
/// with no closing brace yet runs to the end of the file, which is exactly the
/// case an editor is asking about.
export function context(source: string, line: number, column: number): Block[] {
  return (call('val_context', { source, line, column }) as { path: Block[] }).path
}

export type RunResult = {
  action?: string
  wouldNotBuild?: string[]
  error?: string
  outcome?: { kind: 'committed' | 'refused' | 'failed' | 'defect' | 'declined'; why?: string }
  changed?: { path: string; from: unknown; to: unknown }[]
  before?: unknown
  after?: unknown
  effects?: { capability: string; payload: unknown; reversible: boolean }[]
  token?: string
  deviceKey?: string
  record?: {
    codeHash: string
    inputHash: string
    previousRoot: string
    nextRoot: string
    executed: number
    time: number
    uuid: string
    bytes: number
    signature: string
  }
  leaves?: { path: string; value: unknown; hash: string }[]
}

type Exports = {
  memory: WebAssembly.Memory
  val_alloc: (len: number) => number
  val_free: (ptr: number, len: number) => void
  val_analyse: (ptr: number, len: number) => number
  val_words: (ptr: number, len: number) => number
  val_context: (ptr: number, len: number) => number
  val_screen: (ptr: number, len: number) => number
  val_render: (ptr: number, len: number) => number
  val_verify: (ptr: number, len: number) => number
  val_run: (ptr: number, len: number) => number
}

let wasm: Exports | null = null

export async function load(): Promise<void> {
  if (wasm) return
  // `BASE_URL` rather than `/`: this is served under a path on the site, and
  // the same build has to work at both.
  const url = `${import.meta.env.BASE_URL}valang.wasm`
  const { instance } = await WebAssembly.instantiateStreaming(fetch(url), {})
  wasm = instance.exports as unknown as Exports
}

function call(
  fn:
    | 'val_analyse'
    | 'val_render'
    | 'val_screen'
    | 'val_run'
    | 'val_verify'
    | 'val_words'
    | 'val_context',
  input: unknown,
): unknown {
  if (!wasm) throw new Error('the compiler is not loaded')
  const bytes = new TextEncoder().encode(JSON.stringify(input))
  const inPtr = wasm.val_alloc(bytes.length)
  new Uint8Array(wasm.memory.buffer, inPtr, bytes.length).set(bytes)

  const outPtr = wasm[fn](inPtr, bytes.length)
  wasm.val_free(inPtr, bytes.length)

  // Four bytes of little-endian length, then the string.
  const view = new DataView(wasm.memory.buffer)
  const len = view.getUint32(outPtr, true)
  const text = new TextDecoder().decode(new Uint8Array(wasm.memory.buffer, outPtr + 4, len))
  wasm.val_free(outPtr, len + 4)
  return JSON.parse(text)
}

export function analyse(
  source: string,
  text?: Record<string, Record<string, string>>,
  locales?: string[],
  packages?: string[],
): Analysis {
  return call('val_analyse', { source, text, locales, hosts: HOSTS, packages }) as Analysis
}

export function resolve(
  source: string,
  wallet: unknown,
  packages?: string[],
): { screens: Resolved[] } {
  return call('val_render', { source, wallet, packages, hosts: HOSTS }) as { screens: Resolved[] }
}

export type VerifyResult = {
  ok: boolean
  refusal?: { kind: string; why: string }
  record?: { app: string; version: string; action: string; outcome: string; previousRoot: string; nextRoot: string }
  effects?: { capability: string; payload: unknown; reversible: boolean }[]
}

/// What a publisher's server runs. The same crate a Go or a Python SDK will
/// bind to — the editor is not demonstrating a second implementation of the
/// check it is teaching.
export function verifyRecord(token: string, source: string, deviceKey: string): VerifyResult {
  return call('val_verify', { token, source, deviceKey }) as VerifyResult
}

/// One screen, resolved with what a press handed it. A screen that takes
/// parameters cannot be resolved ahead of time, so a host asks for it when it
/// moves.
export function screen(
  source: string,
  name: string,
  wallet: unknown,
  args: Record<string, unknown> = {},
  packages?: string[],
): { name: string; tree: unknown[]; title?: unknown; error?: string } {
  return call('val_screen', { source, screen: name, wallet, args, packages, hosts: HOSTS }) as {
    name: string
    tree: unknown[]
    error?: string
  }
}

/// One action, against the wallet the caller supplies — which is the file
/// somebody can edit, so a run here is a run over data they chose.
export function run(
  source: string,
  action: string,
  wallet: unknown,
  input: Record<string, unknown> = {},
  packages?: string[],
): RunResult {
  return call('val_run', { source, action, wallet, input, packages, hosts: HOSTS }) as RunResult
}
