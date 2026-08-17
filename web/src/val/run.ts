// Build, then run. A simulator, not the runtime: the pipeline in `docs/spec.md`
// §7 is unwritten, and this is what can be shown honestly before it exists.
//
// What it does keep faithfully is the shape — an action is a function of
// (previous state, input, runtime context, code), effects are requested and
// never performed, and the state commits only if the batch would.

import { ExprParser, evalExpr, Trap, type Env, type Expr, type Val } from './expr'
import type { Decl, Program, Tok } from './parse'
import { report, type Finding } from './report'

export type BuildResult = { ok: boolean; problems: Finding[]; checks: { name: string; ok: boolean; note: string }[] }

export type EffectRequest = { capability: string; operation: string; payload: string }
export type PhaseTrace = { phase: string; lines: { text: string; value?: string }[] }
export type RunOutcome = 'committed' | 'refused' | 'defect' | 'outcome'

export type RunResult = {
  action: string
  outcome: RunOutcome
  message?: string
  trace: PhaseTrace[]
  effects: EffectRequest[]
  prevState: Record<string, Val>
  nextState: Record<string, Val>
  prevRoot: string
  nextRoot: string
  context: { time: string; uuid: string }
  leaves: { path: string; value: string; hash: string }[]
}

// --------------------------------------------------------------- the build

export function build(program: Program): BuildResult {
  const rep = report(program)
  const problems: Finding[] = [
    ...program.diagnostics.map((d) => ({ line: d.line, message: d.message, severity: 'error' as const })),
    ...rep.findings,
  ]
  const has = (t: Decl['t']) => program.decls.some((d) => d.t === t)
  const checks = [
    { name: 'parse', ok: program.diagnostics.length === 0, note: 'lexer and shell' },
    { name: 'capabilities', ok: !rep.findings.some((f) => f.message.includes('never used')), note: 'declared and used, one for one' },
    { name: 'determinism', ok: !program.diagnostics.some((d) => d.message.includes('floating')), note: 'no float, no clock of its own' },
    { name: 'effects', ok: !rep.findings.some((f) => f.message.includes('disclosures')), note: 'one batch, at most one disclosure' },
    { name: 'trust', ok: has('trust') || !has('action'), note: 'policies resolve' },
  ]
  return { ok: problems.every((p) => p.severity !== 'error'), problems, checks }
}

// ----------------------------------------------------------------- the run

const parseExpr = (toks: Tok[]) => new ExprParser(toks).parse(0)

// Split a phase body into statements: a new line that is not a continuation.
function statements(toks: Tok[]): Tok[][] {
  const out: Tok[][] = []
  let cur: Tok[] = []
  let depth = 0
  for (const t of toks) {
    if (cur.length && t.line !== cur[cur.length - 1].line && depth === 0) { out.push(cur); cur = [] }
    if ('({['.includes(t.v)) depth++
    if (')}]'.includes(t.v)) depth--
    cur.push(t)
  }
  if (cur.length) out.push(cur)
  return out
}

// A credential nobody signed, shaped like the declaration. It stands in for a
// wallet the playground does not have, and it is visible in the trace so that
// nothing here looks like it came from an issuer.
function mockCredential(name: string, program: Program, seed: number) {
  const decl = program.decls.find((d) => d.t === 'credential' && d.name === name) as any
  const claims: Record<string, Val> = {}
  for (const f of decl?.fields ?? []) {
    claims[f.name] =
      f.type === 'int' ? [12_500, 4_000, 89_900][seed % 3]
      // Times are numbers, because that is what a comparison against
      // `context.time.now` needs. A string here compared false and reported it
      // as an ordinary outcome, which is exactly the wrong lie to tell.
      : f.type === 'datetime' || f.type === 'date' ? Date.parse('2026-08-16T09:12:00Z')
      : f.type === 'string' ? ['Codefin Coffee', 'M-2891', 'TH'][seed % 3]
      : null
  }
  return { claims, signature: { valid: true }, status: { active: true }, holder: { bound: true }, __mock: name }
}

const canon = (v: Val): string =>
  v === null || v === undefined ? 'null'
  : Array.isArray(v) ? `[${v.map(canon).join(',')}]`
  : typeof v === 'object' ? `{${Object.keys(v).filter((k) => !k.startsWith('__')).sort().map((k) => `${JSON.stringify(k)}:${canon(v[k])}`).join(',')}}`
  : JSON.stringify(v)

async function sha(text: string): Promise<string> {
  const buf = await crypto.subtle.digest('SHA-256', new TextEncoder().encode(text))
  return [...new Uint8Array(buf)].map((b) => b.toString(16).padStart(2, '0')).join('')
}

// Leaves are (path, value) pairs sorted by path, the same paths `update`
// patches — spec §7. The canonical encoding here is sorted JSON standing in for
// the dCBOR the real one uses; the shape is right and the bytes are not.
function leavesOf(state: Record<string, Val>, prefix = ''): { path: string; value: string }[] {
  const out: { path: string; value: string }[] = []
  for (const k of Object.keys(state).sort()) {
    const v = state[k]
    const path = prefix ? `${prefix}.${k}` : k
    if (v && typeof v === 'object' && !Array.isArray(v)) out.push(...leavesOf(v, path))
    else out.push({ path, value: canon(v) })
  }
  return out
}

async function merkleRoot(leaves: { path: string; value: string }[]): Promise<{ root: string; hashed: { path: string; value: string; hash: string }[] }> {
  const hashed = await Promise.all(leaves.map(async (l) => ({ ...l, hash: await sha(`${l.path}=${l.value}`) })))
  if (!hashed.length) return { root: await sha(''), hashed }
  let level = hashed.map((h) => h.hash)
  while (level.length > 1) {
    const next: string[] = []
    for (let i = 0; i < level.length; i += 2) next.push(await sha(level[i] + (level[i + 1] ?? level[i])))
    level = next
  }
  return { root: level[0], hashed }
}

export async function run(program: Program, actionName?: string): Promise<RunResult | null> {
  const actions = program.decls.filter((d) => d.t === 'action') as Extract<Decl, { t: 'action' }>[]
  const action = actions.find((a) => a.name === actionName) ?? actions[0]
  if (!action) return null

  const stateDecl = program.decls.find((d) => d.t === 'state') as any
  const trusts = program.decls.filter((d) => d.t === 'trust') as Extract<Decl, { t: 'trust' }>[]

  // A wallet that already has a membership, so the interesting path is taken.
  const prevState: Record<string, Val> = {}
  for (const f of stateDecl?.fields ?? []) {
    if (f.def !== undefined) prevState[f.name] = Number.isNaN(Number(f.def)) ? f.def : Number(f.def)
    else if (f.optional) prevState[f.name] = { member_id: 'M-2891', tier: 'bronze', points: 1_240 }
    else prevState[f.name] = null
  }
  if ('lifetimePoints' in prevState) prevState.lifetimePoints = 1_240

  const context = { time: { now: Date.parse('2026-08-17T10:30:00Z') }, random: { uuid: '0f2a…c71b' } }
  const trace: PhaseTrace[] = []
  const effects: EffectRequest[] = []

  const env: Env = {
    vars: { state: prevState, context },
    fns: {
      duration: (n: any) => Number(n) * 86_400_000,
      tierFor: () => undefined, // replaced below when the program declares it
    },
    policies: new Set(trusts.map((t) => t.name)),
    verified: (subject, policy) => {
      const t = trusts.find((x) => x.name === policy)
      if (!t) throw new Trap(`no trust policy named \`${policy}\``)
      if (!subject) throw new Trap(`nothing to verify against \`${policy}\``)
      return { ...subject, __verified: policy }
    },
  }

  // `duration(days: 30)` — named arguments, and the unit is the name.
  env.fns.duration = (...args: any[]) => Number(args[0]) * 86_400_000

  // Declared functions are pure, so a shallow interpreter over their body is
  // enough — and there is no effect they could be hiding.
  for (const d of program.decls) if (d.t === 'function') env.fns[d.name] = () => undefined

  const finish = async (outcome: RunOutcome, message: string | undefined, nextState: Record<string, Val>) => {
    const prev = await merkleRoot(leavesOf(prevState))
    const next = await merkleRoot(leavesOf(nextState))
    return {
      action: action.name, outcome, message, trace, effects,
      prevState, nextState,
      prevRoot: prev.root, nextRoot: next.root,
      context: { time: new Date(context.time.now).toISOString(), uuid: context.random.uuid },
      leaves: next.hashed,
    } satisfies RunResult
  }

  const order = ['input', 'require', 'verify', 'compute', 'update', 'execute']
  let nextState: Record<string, Val> = { ...prevState }

  for (const phase of order) {
    const toks = action.toks[phase]
    if (!toks?.length) continue
    const lines: PhaseTrace['lines'] = []

    for (const stmt of statements(toks)) {
      const text = stmt.map((t) => (t.k === 'str' ? `"${t.v}"` : t.v)).join(' ').replace(/\s+([.,:)\]])/g, '$1').replace(/([.(\[])\s+/g, '$1')

      try {
        if (phase === 'input') {
          // Read off the tokens, not off the rendered text: `Credential<T>`
          // renders with spaces around the angle brackets and a regex over it
          // silently matched nothing, which showed up three phases later as
          // "nothing to verify".
          const name = stmt[0]?.v
          const gi = stmt.findIndex((t, k) => t.v === 'Credential' && stmt[k + 1]?.v === '<')
          const type = gi >= 0 ? stmt[gi + 2]?.v : undefined
          if (name && type) {
            env.vars[name] = mockCredential(type, program, 0)
            lines.push({ text, value: `a ${type} nobody signed — the playground has no wallet` })
          } else lines.push({ text })
          continue
        }

        if (phase === 'compute' || phase === 'verify') {
          if (stmt[0]?.v === 'const') {
            const name = stmt[1]?.v
            const eq = stmt.findIndex((t) => t.v === '=')
            const value = evalExpr(parseExpr(stmt.slice(eq + 1)), env)
            env.vars[name] = value
            lines.push({ text, value: show(value) })
            continue
          }
        }

        if (phase === 'require' || phase === 'verify') {
          const ok = evalExpr(parseExpr(stmt), env)
          lines.push({ text, value: ok ? 'holds' : 'does not hold' })
          if (!ok) {
            trace.push({ phase, lines })
            return finish(
              phase === 'require' ? 'defect' : 'outcome',
              phase === 'require'
                ? `\`require\` failed. That is a defect in the application, not something to show anybody: the action aborts and nothing commits.`
                : `\`verify\` failed. That is an ordinary outcome — the person is told plainly, and the application has done nothing wrong.`,
              prevState,
            )
          }
          continue
        }

        if (phase === 'update') {
          const colon = stmt.findIndex((t) => t.v === ':')
          const path = stmt.slice(0, colon).map((t) => t.v).join('')
          const value = evalExpr(parseExpr(stmt.slice(colon + 1)), env)
          nextState = patch(nextState, path, value)
          lines.push({ text, value: show(value) })
          continue
        }

        if (phase === 'execute') {
          env.vars.next = nextState
          const head = headOf(stmt)
          if (head) {
            effects.push({
              capability: head,
              operation: head.split('.')[1] ?? head,
              payload: text.slice(head.length).replace(/^\s*\(/, '').replace(/\)\s*$/, '').trim() || '—',
            })
            lines.push({ text, value: 'requested, not performed' })
          } else lines.push({ text })
          continue
        }

        lines.push({ text })
      } catch (err) {
        lines.push({ text, value: err instanceof Trap ? `trap: ${err.message}` : 'could not evaluate' })
        trace.push({ phase, lines })
        return finish('defect', err instanceof Trap ? err.message : 'this simulator could not evaluate that line', prevState)
      }
    }
    trace.push({ phase, lines })
  }

  return finish('committed', undefined, nextState)
}

function headOf(stmt: Tok[]): string | undefined {
  const words: string[] = []
  for (const t of stmt) {
    if (t.k === 'id') words.push(t.v)
    else if (t.v === '.') continue
    else break
    if (t.k === 'id' && words.length >= 2) break
  }
  const head = words.join('.')
  return /^(credential|payment|storage|message|network|disclosure)\./.test(head) || head === 'present' ? head : undefined
}

function patch(state: Record<string, Val>, path: string, value: Val): Record<string, Val> {
  const parts = path.split('.')
  const out = { ...state }
  let cur: any = out
  for (let i = 0; i < parts.length - 1; i++) {
    cur[parts[i]] = { ...(cur[parts[i]] ?? {}) }
    cur = cur[parts[i]]
  }
  cur[parts[parts.length - 1]] = value
  return out
}

function show(v: Val): string {
  if (v === undefined) return '—'
  if (typeof v === 'number' && v > 1_500_000_000_000) return new Date(v).toISOString().slice(0, 16).replace('T', ' ')
  if (typeof v === 'object' && v?.__verified) return `Verified<${v.__verified}>`
  if (typeof v === 'object') return canon(v).slice(0, 60)
  return String(v)
}

export type { Expr }
