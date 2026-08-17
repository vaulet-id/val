// The compiler and the runtime, loaded into the page.
//
// Two exported functions and a length-prefixed string. No binding generator:
// that would be a build step and a version to keep matched, for an interface
// that fits on this page.

export type Diagnostic = {
  line: number
  column: number
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
  val_run: (ptr: number, len: number) => number
}

let wasm: Exports | null = null

export async function load(): Promise<void> {
  if (wasm) return
  const { instance } = await WebAssembly.instantiateStreaming(fetch('/valang.wasm'), {})
  wasm = instance.exports as unknown as Exports
}

function call(fn: 'val_analyse' | 'val_run', input: unknown): unknown {
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
): Analysis {
  return call('val_analyse', { source, text, locales }) as Analysis
}

/// One action, against the wallet the caller supplies — which is the file
/// somebody can edit, so a run here is a run over data they chose.
export function run(source: string, action: string, wallet: unknown): RunResult {
  return call('val_run', { source, action, wallet }) as RunResult
}
