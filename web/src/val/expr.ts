// An expression parser and evaluator for the subset the examples use.
//
// The real one walks a typed AST in Rust. This exists so the playground can
// show what an action *does* rather than only what it says, and it stops
// wherever the examples stop needing it.

import type { Tok } from './parse'

export type Expr =
  | { e: 'num'; v: number }
  | { e: 'str'; v: string }
  | { e: 'id'; v: string }
  | { e: 'member'; obj: Expr; name: string }
  | { e: 'call'; callee: Expr; args: { name?: string; value: Expr }[] }
  | { e: 'unary'; op: string; rhs: Expr }
  | { e: 'binary'; op: string; lhs: Expr; rhs: Expr }
  | { e: 'ternary'; cond: Expr; then: Expr; else: Expr }
  | { e: 'with'; subject: Expr; policy: string }
  | { e: 'exists'; subject: Expr }
  | { e: 'record'; spread?: Expr; fields: { name: string; value: Expr }[] }
  | { e: 'switch'; subject: Expr; arms: { test?: { op: string; rhs: Expr }; match?: Expr; body: Expr }[] }
  | { e: 'lambda'; params: string[]; body: Expr }
  | { e: 'raw'; text: string }

const BINARY: Record<string, number> = {
  '||': 1, '&&': 2,
  '==': 3, '!=': 3, '<': 4, '<=': 4, '>': 4, '>=': 4,
  '+': 5, '-': 5,
  '*': 6, '/': 6, '%': 6,
}

export class ExprParser {
  constructor(private toks: Tok[], private i = 0) {}

  peek(o = 0) { return this.toks[this.i + o] }
  at(v: string, o = 0) { return this.peek(o)?.v === v }
  eat() { return this.toks[this.i++] }
  get pos() { return this.i }
  set pos(v: number) { this.i = v }

  // Two-character operators arrive as two tokens.
  private op(): string | undefined {
    const a = this.peek()?.v, b = this.peek(1)?.v
    if (!a) return undefined
    for (const two of ['==', '!=', '<=', '>=', '&&', '||']) if (a + b === two) return two
    if (a === '=' && b === '>') return undefined
    if (a === '-' && b === '>') return undefined
    if (['+', '-', '*', '/', '%', '<', '>'].includes(a)) return a
    return undefined
  }

  parse(minBp = 0): Expr {
    let lhs = this.unary()
    for (;;) {
      // `receipt with PolicyName` binds looser than arithmetic and tighter than
      // comparison; it appears alone in practice.
      if (this.at('with') && this.peek(1)?.k === 'id') {
        this.eat()
        lhs = { e: 'with', subject: lhs, policy: this.eat().v }
        continue
      }
      if (this.at('exists')) { this.eat(); lhs = { e: 'exists', subject: lhs }; continue }
      const op = this.op()
      if (!op) break
      const bp = BINARY[op]
      if (!bp || bp < minBp) break
      for (let k = 0; k < op.length; k++) this.eat()
      const rhs = this.parse(bp + 1)
      lhs = { e: 'binary', op, lhs, rhs }
    }
    if (this.at('?')) {
      this.eat()
      const then = this.parse(0)
      if (this.at(':')) this.eat()
      const els = this.parse(0)
      return { e: 'ternary', cond: lhs, then, else: els }
    }
    return lhs
  }

  private unary(): Expr {
    if (this.at('-')) { this.eat(); return { e: 'unary', op: '-', rhs: this.unary() } }
    if (this.at('!')) { this.eat(); return { e: 'unary', op: '!', rhs: this.unary() } }
    return this.postfix(this.primary())
  }

  private postfix(base: Expr): Expr {
    for (;;) {
      if (this.at('.') && this.peek(1)?.k === 'id') { this.eat(); base = { e: 'member', obj: base, name: this.eat().v }; continue }
      if (this.at('(')) { base = { e: 'call', callee: base, args: this.args() }; continue }
      // `list.fold(0) { sum, h -> … }` — a trailing block is one more argument.
      if (this.at('{') && base.e === 'call') {
        const lam = this.lambda()
        base = { ...base, args: [...base.args, { value: lam }] }
        continue
      }
      break
    }
    return base
  }

  private args(): { name?: string; value: Expr }[] {
    this.eat() // (
    const out: { name?: string; value: Expr }[] = []
    while (this.peek() && !this.at(')')) {
      let name: string | undefined
      if (this.peek()?.k === 'id' && this.at(':', 1)) { name = this.eat().v; this.eat() }
      out.push({ name, value: this.parse(0) })
      if (this.at(',')) this.eat()
      else break
    }
    if (this.at(')')) this.eat()
    return out
  }

  private lambda(): Expr {
    this.eat() // {
    const params: string[] = []
    const save = this.i
    while (this.peek() && !this.at('-') && !this.at('}')) {
      if (this.peek().k === 'id') params.push(this.eat().v)
      else if (this.at(',')) this.eat()
      else break
    }
    if (this.at('-') && this.at('>', 1)) { this.eat(); this.eat() }
    else { this.i = save; params.length = 0 }
    const body = this.parse(0)
    if (this.at('}')) this.eat()
    return { e: 'lambda', params, body }
  }

  private primary(): Expr {
    const t = this.peek()
    if (!t) return { e: 'raw', text: '' }

    if (t.k === 'num') { this.eat(); return { e: 'num', v: Number(t.v.replace(/_/g, '')) } }
    if (t.k === 'str') { this.eat(); return { e: 'str', v: t.v } }

    if (this.at('(')) { this.eat(); const inner = this.parse(0); if (this.at(')')) this.eat(); return inner }

    if (this.at('switch')) {
      this.eat()
      if (this.at('(')) this.eat()
      const subject = this.parse(0)
      if (this.at(')')) this.eat()
      if (this.at('{')) this.eat()
      const arms: Extract<Expr, { e: 'switch' }>['arms'] = []
      while (this.peek() && !this.at('}')) {
        if (this.at('default')) {
          this.eat()
          if (this.at('=') && this.at('>', 1)) { this.eat(); this.eat() }
          arms.push({ body: this.parse(0) })
        } else {
          const op = this.op()
          if (op) {
            for (let k = 0; k < op.length; k++) this.eat()
            const rhs = this.parse(0)
            if (this.at('=') && this.at('>', 1)) { this.eat(); this.eat() }
            arms.push({ test: { op, rhs }, body: this.parse(0) })
          } else {
            const match = this.parse(0)
            if (this.at('=') && this.at('>', 1)) { this.eat(); this.eat() }
            arms.push({ match, body: this.parse(0) })
          }
        }
        if (this.at(',')) this.eat()
      }
      if (this.at('}')) this.eat()
      return { e: 'switch', subject, arms }
    }

    if (this.at('{')) {
      // A record literal, possibly spread: { ...state.member, tier: t }
      this.eat()
      let spread: Expr | undefined
      const fields: { name: string; value: Expr }[] = []
      while (this.peek() && !this.at('}')) {
        if (this.at('.') && this.at('.', 1) && this.at('.', 2)) { this.eat(); this.eat(); this.eat(); spread = this.parse(0) }
        else if (this.peek()?.k === 'id' && this.at(':', 1)) {
          const name = this.eat().v
          this.eat()
          fields.push({ name, value: this.parse(0) })
        } else this.eat()
        if (this.at(',')) this.eat()
      }
      if (this.at('}')) this.eat()
      return { e: 'record', spread, fields }
    }

    if (t.k === 'id') { this.eat(); return { e: 'id', v: t.v } }
    this.eat()
    return { e: 'raw', text: t.v }
  }
}

// ------------------------------------------------------------------ values

export type Val = any

export class Trap extends Error {}

export type Env = {
  vars: Record<string, Val>
  fns: Record<string, (args: Val[]) => Val>
  policies: Set<string>
  verified: (subject: Val, policy: string) => Val   // throws to fail as an outcome
}

export function evalExpr(x: Expr, env: Env): Val {
  switch (x.e) {
    case 'num': return x.v
    case 'str': return x.v
    case 'id': {
      if (x.v in env.vars) return env.vars[x.v]
      if (x.v in env.fns) return env.fns[x.v]
      return undefined
    }
    case 'member': {
      const obj = evalExpr(x.obj, env)
      if (obj == null) throw new Trap(`\`${describe(x.obj)}\` may not exist — narrow it in \`require\` first`)
      return obj[x.name]
    }
    case 'unary': {
      const v = evalExpr(x.rhs, env)
      return x.op === '-' ? -v : !v
    }
    case 'binary': {
      const a = evalExpr(x.lhs, env)
      const b = evalExpr(x.rhs, env)
      switch (x.op) {
        case '+': return checked(a + b)
        case '-': return checked(a - b)
        case '*': return checked(a * b)
        case '/':
          if (b === 0) throw new Trap('division by zero traps, as overflow does')
          return Math.trunc(a / b)
        case '%': return a % b
        case '<': return a < b
        case '<=': return a <= b
        case '>': return a > b
        case '>=': return a >= b
        case '==': return a === b
        case '!=': return a !== b
        case '&&': return a && b
        case '||': return a || b
      }
      return undefined
    }
    case 'ternary': return evalExpr(x.cond, env) ? evalExpr(x.then, env) : evalExpr(x.else, env)
    case 'exists': {
      try { return evalExpr(x.subject, env) != null } catch { return false }
    }
    case 'with': return env.verified(evalExpr(x.subject, env), x.policy)
    case 'record': {
      const base = x.spread ? { ...evalExpr(x.spread, env) } : {}
      for (const f of x.fields) base[f.name] = evalExpr(f.value, env)
      return base
    }
    case 'switch': {
      const s = evalExpr(x.subject, env)
      for (const arm of x.arms) {
        if (arm.test) {
          const r = evalExpr(arm.test.rhs, env)
          const ok =
            arm.test.op === '>=' ? s >= r : arm.test.op === '>' ? s > r :
            arm.test.op === '<=' ? s <= r : arm.test.op === '<' ? s < r :
            arm.test.op === '==' ? s === r : false
          if (ok) return evalExpr(arm.body, env)
        } else if (arm.match) {
          if (evalExpr(arm.match, env) === s) return evalExpr(arm.body, env)
        } else return evalExpr(arm.body, env)
      }
      return undefined
    }
    case 'lambda': return (...args: Val[]) => {
      const inner: Env = { ...env, vars: { ...env.vars } }
      x.params.forEach((p, i) => (inner.vars[p] = args[i]))
      return evalExpr(x.body, inner)
    }
    case 'call': {
      const args = x.args.map((a) => evalExpr(a.value, env))
      if (x.callee.e === 'member') {
        const recv = evalExpr(x.callee.obj, env)
        const name = x.callee.name
        if (Array.isArray(recv)) {
          if (name === 'fold') return recv.reduce((acc, item) => args[1](acc, item), args[0])
          if (name === 'map') return recv.map((item) => args[0](item))
          if (name === 'filter') return recv.filter((item) => args[0](item))
          if (name === 'any') return recv.some((item) => args[0](item))
          if (name === 'all') return recv.every((item) => args[0](item))
        }
        const fn = recv?.[name]
        return typeof fn === 'function' ? fn(...args) : undefined
      }
      const fn = evalExpr(x.callee, env)
      return typeof fn === 'function' ? fn(...args) : undefined
    }
    default: return undefined
  }
}

// `int` is 64-bit and traps on overflow (spec §3). JavaScript numbers stop being
// integers well before that, so the check is against the safe range and says so.
function checked(n: number): number {
  if (!Number.isSafeInteger(n)) throw new Trap('integer overflow traps: a wrong number the record would then faithfully prove is worse than a failure')
  return n
}

export function describe(x: Expr): string {
  switch (x.e) {
    case 'id': return x.v
    case 'member': return `${describe(x.obj)}.${x.name}`
    default: return 'value'
  }
}
