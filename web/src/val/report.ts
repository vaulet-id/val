// The capability report, derived from the code rather than declared by its
// author (spec §7). Every line here is answerable statically only because of
// decisions taken for other reasons — effects live in `execute`, strings cannot
// be assembled, audiences are fixed in the manifest, the language is total.

import type { Decl, Node, Program } from './parse'

export type ReportLine = { label: string; values: string[]; tone?: 'plain' | 'warn' }
export type Finding = { line: number; message: string; severity: 'error' | 'warning' }

export type Report = {
  app: string
  version: string
  lines: ReportLine[]
  findings: Finding[]
}

const flatten = (nodes: Node[]): Node[] => nodes.flatMap((n) => [n, ...flatten(n.children)])

export function report(program: Program): Report {
  const { decls } = program
  const app = (decls.find((d) => d.t === 'app') as any)?.id ?? '—'
  const version = (decls.find((d) => d.t === 'version') as any)?.value ?? '—'
  const caps = (decls.find((d) => d.t === 'capabilities') as any)?.entries ?? ([] as Node[])
  const actions = decls.filter((d) => d.t === 'action') as Extract<Decl, { t: 'action' }>[]
  const screens = decls.filter((d) => d.t === 'screen') as Extract<Decl, { t: 'screen' }>[]
  const trusts = decls.filter((d) => d.t === 'trust') as Extract<Decl, { t: 'trust' }>[]

  const effects = actions.flatMap((a) => flatten(Object.values(a.phases).flat()))
  const findingsEarly: Finding[] = []
  const findings: Finding[] = []
  const used = new Set<string>()

  // reads --------------------------------------------------------------------
  const reads = new Set<string>()
  for (const s of screens)
    for (const d of s.data)
      if (d.source === 'credentials' && d.credentialType)
        reads.add(`${d.credentialType}${d.policy ? ` under ${d.policy}` : ' — unverified'}`)
  for (const a of actions) {
    const v = a.raw['verify'] ?? ''
    for (const t of trusts) if (v.includes(t.name)) reads.add(`${t.subjectType || '?'} under ${t.name}`)
    const inp = a.raw['input'] ?? ''
    const m = inp.match(/Credential<\s*(\w+)/)
    if (m && ![...reads].some((r) => r.startsWith(m[1]))) reads.add(`${m[1]} — unverified`)
  }
  if (reads.size) used.add('credential.read')

  // discloses / proves -------------------------------------------------------
  // Counted per action, because the rule is per action: a second disclosure
  // cannot be conditional on a batch the first has already completed (spec §5).
  const discloses: string[] = []
  const proves: string[] = []
  for (const a of actions) {
    let here = 0
    for (const line of (a.raw['execute'] ?? '').split('\n')) {
      const d = line.match(/\bdisclose\s+(.+)/)
      if (d) { discloses.push(d[1].trim()); here++ }
      const pr = line.match(/\bprove\s+(.+)/)
      if (pr) { proves.push(pr[1].trim()); here++ }
    }
    if (here > 1)
      findingsEarly.push({
        line: a.line,
        severity: 'error',
        message: `\`${a.name}\` performs ${here} disclosures. An action performs at most one: a second cannot be conditional on a batch the first has already completed.`,
      })
  }
  if (discloses.length || proves.length) used.add('disclosure.present')

  // issues -------------------------------------------------------------------
  const issues = new Set<string>()
  for (const n of effects) if (n.kind === 'credential.issue') issues.add(n.args[0]?.value.split(/[\s{]/)[0] ?? '?')
  if (issues.size) used.add('credential.issue')

  // audiences ----------------------------------------------------------------
  const audiences = new Set<string>()
  for (const c of caps as Node[]) {
    if (c.kind === 'api.query') {
      const a = c.args.find((x) => x.name === 'audience')
      if (a) audiences.add(a.value.replace(/"/g, ''))
    }
  }
  for (const s of screens) for (const d of s.data) if (d.source === 'query' && d.audience) audiences.add(d.audience)
  if (audiences.size) used.add('api.query')

  // payments -----------------------------------------------------------------
  const payments: string[] = []
  for (const n of effects) if (n.kind === 'payment.request') payments.push(n.args.map((a) => `${a.name ? a.name + ': ' : ''}${a.value}`).join(', '))
  if (payments.length) used.add('payment.request')

  // state --------------------------------------------------------------------
  const writes = new Set<string>()
  for (const a of actions)
    for (const line of (a.raw['update'] ?? '').split('\n')) {
      const m = line.match(/^\s*([\w.]+)\s*:/)
      if (m) writes.add(m[1])
    }

  // findings -----------------------------------------------------------------
  for (const c of caps as Node[]) {
    const head = c.kind.split('(')[0]
    if (!used.has(head))
      findings.push({
        line: c.line,
        severity: 'error',
        message: `\`${c.kind}\` is declared and never used. Consent asked for something unused is consent spent on nothing, and it trains people to say yes.`,
      })
  }
  findings.push(...findingsEarly)
  for (const s of screens)
    for (const d of s.data)
      if (d.source === 'credentials' && !d.policy)
        findings.push({ line: d.line, severity: 'warning', message: `\`${d.name}\` is read without \`verified with\`, so the list can show a credential nothing vouched for.` })

  const lines: ReportLine[] = [
    { label: 'reads', values: [...reads] },
    { label: 'discloses', values: discloses.length ? discloses : ['nothing'] },
    { label: 'proves', values: proves },
    { label: 'issues', values: [...issues] },
    { label: 'talks to', values: [...audiences] },
    { label: 'moves money', values: payments, tone: 'warn' },
    { label: 'writes state', values: [...writes] },
    {
      label: 'irreversible',
      values: discloses.length || proves.length ? ['one disclosure'] : ['none'],
      tone: discloses.length ? 'warn' : 'plain',
    },
  ]

  return { app, version, lines, findings }
}
