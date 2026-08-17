// A parser for enough of VAL to be useful before the real one exists.
//
// It is deliberately not the specification's parser: it is written in the
// browser's language so the playground can run with no backend, and it stops at
// the point where a preview and a capability report can be produced. Where it
// disagrees with `docs/spec.md`, the specification is right and this is a bug.

export type Diagnostic = { line: number; column: number; message: string }

export type Arg = { name?: string; value: string; line: number }
export type Node = {
  kind: string          // the head word: column, card, tab, list, button, …
  args: Arg[]
  children: Node[]
  lambda?: string       // `list(x) { row -> … }`
  line: number
}

export type Decl =
  | { t: 'app'; id: string; line: number }
  | { t: 'version'; value: string; line: number }
  | { t: 'capabilities'; entries: Node[]; line: number }
  | { t: 'enum'; name: string; members: string[]; line: number }
  | { t: 'credential'; name: string; fields: Field[]; line: number }
  | { t: 'state'; fields: Field[]; line: number }
  | { t: 'trust'; name: string; subject: string; subjectType: string; refines?: string; anchor?: string; requires: string[]; line: number }
  | { t: 'function'; name: string; line: number }
  | { t: 'action'; name: string; phases: Record<string, Node[]>; raw: Record<string, string>; toks: Record<string, Tok[]>; line: number }
  | { t: 'screen'; name: string; data: DataDecl[]; compute: string[]; tree: Node[]; line: number }

export type Field = { name: string; type: string; optional: boolean; def?: string; line: number }
export type DataDecl = {
  name: string
  source: 'credentials' | 'query' | 'unknown'
  credentialType?: string
  policy?: string
  audience?: string
  modifiers: string[]
  line: number
}

export type Program = { decls: Decl[]; diagnostics: Diagnostic[] }

// ---------------------------------------------------------------- tokenizer

export type Tok = { v: string; k: 'id' | 'str' | 'num' | 'punct'; line: number; col: number }

const PUNCT = new Set([
  '{', '}', '(', ')', '[', ']', ',', ':', '.', ';', '?',
  // Operators. Arithmetic included: `amount / satangPerBaht` is ordinary VAL,
  // and leaving `/` out made the lexer call it an unexpected character and then
  // blame Thai for it — a wrong message is worse than no message.
  '+', '-', '*', '/', '%', '<', '>', '=', '!', '&', '|',
])

export function lex(src: string): { toks: Tok[]; diagnostics: Diagnostic[] } {
  const toks: Tok[] = []
  const diagnostics: Diagnostic[] = []
  let i = 0, line = 1, col = 1
  const push = (v: string, k: Tok['k'], l = line, c = col) => toks.push({ v, k, line: l, col: c })

  while (i < src.length) {
    const ch = src[i]
    if (ch === '\n') { i++; line++; col = 1; continue }
    if (ch === ' ' || ch === '\t' || ch === '\r') { i++; col++; continue }
    if (ch === '/' && src[i + 1] === '/') { while (i < src.length && src[i] !== '\n') i++; continue }

    if (ch === '"') {
      const startLine = line, startCol = col
      let out = ''
      i++; col++
      while (i < src.length && src[i] !== '"') {
        if (src[i] === '\n') { diagnostics.push({ line: startLine, column: startCol, message: 'unterminated string' }); break }
        out += src[i]; i++; col++
      }
      i++; col++
      push(out, 'str', startLine, startCol)
      continue
    }

    if (/[0-9]/.test(ch)) {
      const startCol = col
      let out = ''
      while (i < src.length && /[0-9_.]/.test(src[i])) { out += src[i]; i++; col++ }
      // The lexer takes the whole thing so the message can be about floats
      // rather than about an unexpected field name — spec §2.
      if (out.includes('.')) diagnostics.push({ line, column: startCol, message: `no floating-point type in this language: \`${out}\`. Use a smaller unit — satang, micro-shares.` })
      if (/^_|_$/.test(out.replace(/\./g, ''))) diagnostics.push({ line, column: startCol, message: `\`_\` separates digits, it does not start or end a number` })
      push(out, 'num', line, startCol)
      continue
    }

    if (/[A-Za-z_]/.test(ch)) {
      const startCol = col
      let out = ''
      while (i < src.length && /[A-Za-z0-9_]/.test(src[i])) { out += src[i]; i++; col++ }
      push(out, 'id', line, startCol)
      continue
    }

    if (PUNCT.has(ch)) { push(ch, 'punct'); i++; col++; continue }

    // Two different mistakes deserve two different sentences.
    diagnostics.push({
      line,
      column: col,
      message:
        ch.codePointAt(0)! > 127
          ? `identifiers are ASCII, and \`${ch}\` is not. Thai belongs in strings and in the manifest's text bundle — spec §2.`
          : `unexpected character \`${ch}\``,
    })
    i++; col++
  }
  return { toks, diagnostics }
}

// ------------------------------------------------------------------- parser

export function parse(src: string): Program {
  const { toks, diagnostics } = lex(src)
  const decls: Decl[] = []
  let p = 0

  const peek = (o = 0) => toks[p + o]
  const at = (v: string, o = 0) => peek(o)?.v === v
  const eat = () => toks[p++]
  const expect = (v: string) => {
    if (at(v)) return eat()
    const t = peek()
    diagnostics.push({ line: t?.line ?? 1, column: t?.col ?? 1, message: `expected \`${v}\`${t ? `, found \`${t.v}\`` : ' before end of file'}` })
    return undefined
  }

  // Collect the raw source of a balanced { … } so phases keep their text.
  const skipBlock = (): Tok[] => {
    const out: Tok[] = []
    if (!at('{')) return out
    let depth = 0
    do {
      const t = eat()
      if (!t) break
      if (t.v === '{') depth++
      else if (t.v === '}') depth--
      if (depth > 0 && !(depth === 1 && t.v === '{' && out.length === 0)) out.push(t)
    } while (depth > 0 && p < toks.length)
    return out
  }

  // `credential.read` is one head word; a dot is field access everywhere else
  // (spec §2), and here it is the same rule read as a name.
  const dotted = (): string => {
    if (!peek() || peek().k !== 'id') return ''
    let s = eat().v
    while (at('.') && peek(1)?.k === 'id') { eat(); s += '.' + eat().v }
    return s
  }

  const parseArgs = (): Arg[] => {
    const args: Arg[] = []
    if (!at('(')) return args
    eat()
    let depth = 1
    let cur = ''
    let name: string | undefined
    const line = peek()?.line ?? 1
    while (p < toks.length && depth > 0) {
      const t = eat()
      if (t.v === '(') { depth++; cur += '(' ; continue }
      if (t.v === ')') { depth--; if (depth === 0) break; cur += ')'; continue }
      if (t.v === ',' && depth === 1) { if (cur.trim()) args.push({ name, value: tidy(cur), line }); cur = ''; name = undefined; continue }
      if (t.v === ':' && depth === 1 && cur.trim() && !cur.trim().includes(' ') && !name) { name = cur.trim(); cur = ''; continue }
      cur += (t.k === 'str' ? `"${t.v}"` : t.v) + (t.k === 'id' || t.k === 'num' ? ' ' : '')
    }
    if (cur.trim()) args.push({ name, value: cur.trim(), line })
    return args.map((a) => ({ ...a, value: tidy(a.value) }))
  }

  // A UI node, a capability entry, an effect — all the same shape:
  //   head [ ( args ) ] [ { children } ]
  const parseNode = (): Node | undefined => {
    const start = peek()
    if (!start || start.k !== 'id') { eat(); return undefined }
    const line = start.line
    const kind = dotted()
    const args = parseArgs()
    let lambda: string | undefined
    const children: Node[] = []
    if (at('{')) {
      eat()
      if (peek()?.k === 'id' && at('-', 1) && at('>', 2)) { lambda = eat().v; eat(); eat() }
      while (p < toks.length && !at('}')) {
        const before = p
        const n = parseNode()
        if (n) children.push(n)
        if (p === before) eat()
      }
      expect('}')
    }
    return { kind, args, children, line }
  }

  const parseFields = (): Field[] => {
    const fields: Field[] = []
    expect('{')
    while (p < toks.length && !at('}')) {
      if (peek().k !== 'id') { eat(); continue }
      const line = peek().line
      const name = eat().v
      if (!at(':')) { continue }
      eat()
      let type = ''
      while (p < toks.length && !at('}') && peek().line === line && !at('default')) {
        const t = eat()
        type += t.v
      }
      let def: string | undefined
      if (at('default')) { eat(); def = eat()?.v }
      const optional = type.endsWith('?')
      fields.push({ name, type: optional ? type.slice(0, -1) : type, optional, def, line })
    }
    expect('}')
    return fields
  }

  while (p < toks.length) {
    const t = peek()
    if (!t) break
    const line = t.line

    if (t.v === 'app') { eat(); decls.push({ t: 'app', id: peek()?.v ?? '', line }); eat(); continue }
    if (t.v === 'version') { eat(); decls.push({ t: 'version', value: eat()?.v ?? '', line }); continue }

    if (t.v === 'capabilities') {
      eat(); expect('{')
      const entries: Node[] = []
      while (p < toks.length && !at('}')) {
        const before = p
        const n = parseNode()
        if (n) entries.push(n)
        if (p === before) eat()
      }
      expect('}')
      decls.push({ t: 'capabilities', entries, line })
      continue
    }

    if (t.v === 'enum') {
      eat()
      const name = eat()?.v ?? ''
      expect('{')
      const members: string[] = []
      while (p < toks.length && !at('}')) { const m = eat(); if (m.k === 'id') members.push(m.v) }
      expect('}')
      decls.push({ t: 'enum', name, members, line })
      continue
    }

    if (t.v === 'credential') {
      eat()
      const name = eat()?.v ?? ''
      decls.push({ t: 'credential', name, fields: parseFields(), line })
      continue
    }

    if (t.v === 'state') { eat(); decls.push({ t: 'state', fields: parseFields(), line }); continue }

    if (t.v === 'trust') {
      eat()
      const name = eat()?.v ?? ''
      let subject = '', subjectType = ''
      if (at('(')) {
        eat()
        subject = eat()?.v ?? ''
        if (at(':')) { eat(); subjectType = eat()?.v ?? '' }
        expect(')')
      }
      let refines: string | undefined
      if (at('refines')) { eat(); refines = eat()?.v }
      const body = skipBlock()
      let anchor: string | undefined
      const requires: string[] = []
      for (let k = 0; k < body.length; k++) {
        if (body[k].v === 'anchor' && body[k + 1]?.v === ':') anchor = body[k + 2]?.v
        if (body[k].v === 'require' && body[k + 1]?.v === '{') {
          let depth = 0, cur = '', curLine = -1
          for (let j = k + 1; j < body.length; j++) {
            const b = body[j]
            if (b.v === '{') { depth++; if (depth === 1) continue }
            if (b.v === '}') { depth--; if (depth === 0) { if (cur.trim()) requires.push(cur.trim()); break } }
            if (curLine !== -1 && b.line !== curLine) { if (cur.trim()) requires.push(cur.trim()); cur = '' }
            curLine = b.line
            cur += b.v
          }
          break
        }
      }
      decls.push({ t: 'trust', name, subject, subjectType, refines, anchor, requires, line })
      continue
    }

    if (t.v === 'function') {
      eat()
      const name = eat()?.v ?? ''
      parseArgs()
      if (at(':')) { eat(); eat() }
      skipBlock()
      decls.push({ t: 'function', name, line })
      continue
    }

    if (t.v === 'action') {
      eat()
      const name = eat()?.v ?? ''
      const phases: Record<string, Node[]> = {}
      const raw: Record<string, string> = {}
      const phaseToks: Record<string, Tok[]> = {}
      expect('{')
      while (p < toks.length && !at('}')) {
        if (peek().k !== 'id') { eat(); continue }
        const phase = eat().v
        const body = skipBlock()
        raw[phase] = renderToks(body)
        phaseToks[phase] = body
        // Effects are found by head word, so the body is walked as nodes too.
        phases[phase] = nodesFromToks(body)
      }
      expect('}')
      decls.push({ t: 'action', name, phases, raw, toks: phaseToks, line })
      continue
    }

    if (t.v === 'screen') {
      eat()
      const name = eat()?.v ?? ''
      expect('{')
      const data: DataDecl[] = []
      const compute: string[] = []
      const tree: Node[] = []
      while (p < toks.length && !at('}')) {
        if (at('data') && at('{', 1)) { eat(); data.push(...parseData(skipBlock())); continue }
        if (at('compute') && at('{', 1)) { eat(); compute.push(...linesOf(skipBlock())); continue }
        const before = p
        const n = parseNode()
        if (n) tree.push(n)
        if (p === before) eat()
      }
      expect('}')
      decls.push({ t: 'screen', name, data, compute, tree, line })
      continue
    }

    eat()
  }

  return { decls, diagnostics }
}

// Re-emit tokens as text that still reads like VAL: a space between two words,
// none around punctuation. Report extraction greps this, and `member .points`
// would quietly match nothing.
// `h .claims .symbol` is the token stream; `h.claims.symbol` is what somebody
// wrote and what a preview should show back to them.
export const tidy = (s: string) =>
  s.replace(/\s+([.,)\]:?])/g, '$1').replace(/([.(\[])\s+/g, '$1').trim()

const TIGHT_BEFORE = new Set(['.', ',', ':', ')', ']', '}', '?', ';'])
const TIGHT_AFTER = new Set(['.', '(', '[', '{'])

function renderToks(toks: Tok[]): string {
  let out = '', line = toks[0]?.line ?? 1
  for (let i = 0; i < toks.length; i++) {
    const t = toks[i]
    while (t.line > line) { out += '\n'; line++ }
    const text = t.k === 'str' ? `"${t.v}"` : t.v
    const prev = toks[i - 1]
    const needsSpace =
      out.length > 0 &&
      !out.endsWith('\n') &&
      !TIGHT_BEFORE.has(t.v) &&
      !(prev && TIGHT_AFTER.has(prev.v))
    out += (needsSpace ? ' ' : '') + text
  }
  return out
}

function linesOf(toks: Tok[]): string[] {
  const out: string[] = []
  let cur = '', line = -1
  for (const t of toks) {
    if (line !== -1 && t.line !== line) { if (cur.trim()) out.push(cur.trim()); cur = '' }
    line = t.line
    cur += (t.k === 'str' ? `"${t.v}"` : t.v) + (t.k === 'id' ? ' ' : '')
  }
  if (cur.trim()) out.push(cur.trim())
  return out
}

// Effects and UI nodes both read as `head(args) { children }`; reuse one walk.
function nodesFromToks(toks: Tok[]): Node[] {
  const src = renderToks(toks)
  const { decls } = parse(`screen __ {\n${src}\n}`)
  const screen = decls.find((d) => d.t === 'screen') as Extract<Decl, { t: 'screen' }> | undefined
  return screen ? screen.tree : []
}

function parseData(toks: Tok[]): DataDecl[] {
  const out: DataDecl[] = []
  let i = 0
  while (i < toks.length) {
    if (toks[i].k !== 'id' || toks[i + 1]?.v !== ':') { i++; continue }
    const name = toks[i].v
    const line = toks[i].line
    i += 2
    const start = i
    while (i < toks.length && !(toks[i].k === 'id' && toks[i + 1]?.v === ':' && toks[i].line !== toks[start]?.line)) i++
    const body = toks.slice(start, i)
    const words = body.map((t) => t.v)
    const dec: DataDecl = { name, source: 'unknown', modifiers: [], line }
    if (words[0] === 'credentials') {
      dec.source = 'credentials'
      dec.credentialType = words[2]
      const vi = words.indexOf('with')
      if (vi >= 0) dec.policy = words[vi + 1]
    } else if (words[0] === 'query') {
      dec.source = 'query'
      dec.audience = words[1]
    }
    for (const m of ['order', 'limit']) {
      const mi = words.indexOf(m)
      if (mi >= 0) dec.modifiers.push(words.slice(mi, mi + 4).join(' '))
    }
    out.push(dec)
  }
  return out
}
