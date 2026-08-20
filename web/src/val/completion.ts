// What the editor offers, and where it gets it.
//
// Monaco's default is every word it has seen in the file, which offers
// `merchant` where a keyword belongs and never offers `credentials of` at all.
// Everything here comes from the two documents that decide what is legal: the
// host's registry, and the compiler — the keywords from `val_words`, the names
// from the analysis that just ran. Nothing is typed out twice, because a list
// written here is a list that drifts the day the language changes.

import type * as Monaco from 'monaco-editor'

import { HOSTS, words, type Declared } from './wasm'

type Capability = {
  draws?: boolean
  children?: boolean
  primary?: string
  props?: Record<string, string>
  requires?: string
}

type Registry = {
  capabilities: Record<string, Capability>
  common?: Record<string, Record<string, string>>
  screen?: { props?: Record<string, string> }
  vocabularies?: Record<string, { words: string[]; open: boolean }>
}

const registries: Registry[] = HOSTS.map((h) => JSON.parse(h) as Registry)

/// Layout, accessibility and style, which every drawn thing carries.
const common = (): Record<string, string> => {
  const out: Record<string, string> = {}
  for (const r of registries) {
    for (const group of Object.values(r.common ?? {})) {
      if (typeof group === 'object') Object.assign(out, group)
    }
  }
  return out
}

const capability = (kind: string): Capability | undefined => {
  for (const r of registries) {
    const found = r.capabilities?.[kind]
    if (found) return found
  }
  return undefined
}

/// `ColorToken?` is the vocabulary `colorToken`.
const vocabulary = (ty: string) => {
  const key = ty.replace(/\?$/, '')
  const name = key.charAt(0).toLowerCase() + key.slice(1)
  for (const r of registries) {
    const found = r.vocabularies?.[name]
    if (found) return found
  }
  return undefined
}

/// The node a property belongs to: the nearest line above that opens a block
/// and is less indented than this one.
function enclosingKind(lines: string[], at: number): string | undefined {
  const indent = (s: string) => s.length - s.trimStart().length
  const mine = indent(lines[at] ?? '')
  for (let i = at - 1; i >= 0; i--) {
    const line = lines[i]
    if (!line.trim() || line.trimStart().startsWith('//')) continue
    if (indent(line) >= mine) continue
    const opens = /^\s*([a-zA-Z][\w.]*)\s*(\([^)]*\))?\s*\{/.exec(line)
    if (opens) return opens[1]
    return undefined
  }
  return undefined
}

/// Which block the cursor is in, by the nearest unclosed opener above it.
function enclosingBlock(before: string): string | undefined {
  const stack: string[] = []
  for (const raw of before.split('\n')) {
    const line = raw.split('//')[0]
    const opener = /^\s*(?:@\w+\s*)?([a-zA-Z][\w.]*)/.exec(line)?.[1]
    for (const ch of line) {
      if (ch === '{') stack.push(opener ?? '')
      else if (ch === '}') stack.pop()
    }
  }
  return stack.filter(Boolean).pop()
}

export function registerCompletion(monaco: typeof Monaco, declared: () => Declared) {
  monaco.languages.registerCompletionItemProvider('val', {
    triggerCharacters: [' ', ':', '.', '@'],

    provideCompletionItems(model, position) {
      const w = words()
      const names = declared()
      const line = model.getLineContent(position.lineNumber)
      const prefix = line.slice(0, position.column - 1)
      const before = model.getValueInRange({
        startLineNumber: 1,
        startColumn: 1,
        endLineNumber: position.lineNumber,
        endColumn: position.column,
      })
      const lines = model.getValue().split('\n')
      const word = model.getWordUntilPosition(position)
      const range = {
        startLineNumber: position.lineNumber,
        endLineNumber: position.lineNumber,
        startColumn: word.startColumn,
        endColumn: word.endColumn,
      }

      const Kind = monaco.languages.CompletionItemKind
      const item = (
        label: string,
        kind: Monaco.languages.CompletionItemKind,
        detail: string,
        insert = label,
      ): Monaco.languages.CompletionItem => ({
        label,
        kind,
        detail,
        insertText: insert,
        insertTextRules:
          insert === label
            ? undefined
            : monaco.languages.CompletionItemInsertTextRule.InsertAsSnippet,
        range,
      })

      const block = enclosingBlock(before)

      // `credentials of ‹type›` and `verified with ‹policy›`, which name things
      // this program declares and nothing else.
      if (/\bof\s+\w*$/.test(prefix) && /credentials\b/.test(prefix)) {
        return {
          suggestions: names.credentials.map((c) =>
            item(c, Kind.Class, 'a credential this package declares'),
          ),
        }
      }
      if (/\bwith\s+\w*$/.test(prefix)) {
        return {
          suggestions: names.trusts.map((t) => item(t, Kind.Interface, 'a policy')),
        }
      }

      // A property's value. What may go there is the registry's answer.
      const prop = /([a-zA-Z]\w*)\s*:\s*\w*$/.exec(prefix)?.[1]
      if (prop && block) {
        const kind = enclosingKind(lines, position.lineNumber - 1) ?? block
        const ty = capability(kind)?.props?.[prop] ?? common()[prop]
        if (ty) {
          const bare = ty.replace(/\?$/, '')
          if (bare === 'Action' || bare === 'Screen') {
            return {
              suggestions: [
                ...names.screens.map((s) => item(s, Kind.Function, 'a screen this press moves to')),
                ...declaredActions(names).map((a) => item(a, Kind.Event, 'an action')),
              ],
            }
          }
          const vocab = vocabulary(bare)
          if (vocab) {
            return {
              suggestions: vocab.words.map((v) =>
                item(v, Kind.EnumMember, vocab.open ? `${bare} — or one of your own` : bare),
              ),
            }
          }
          if (bare === 'Text') {
            return {
              suggestions: [
                item('phrase', Kind.Snippet, 'words and the values in them', 'phrase("$1", $2)'),
              ],
            }
          }
        }
      }

      // Inside `capabilities { … }`: everything this host does.
      if (block === 'capabilities') {
        const out: Monaco.languages.CompletionItem[] = []
        for (const r of registries) {
          for (const [name, cap] of Object.entries(r.capabilities ?? {})) {
            if (cap.draws) continue
            out.push(item(name, Kind.Module, 'a capability this host provides'))
          }
        }
        return { suggestions: out }
      }

      // At the start of a line inside something that draws: the components.
      if (block && (capability(block)?.children || block === 'column' || isBody(block, names))) {
        const out: Monaco.languages.CompletionItem[] = []
        for (const r of registries) {
          for (const [name, cap] of Object.entries(r.capabilities ?? {})) {
            if (!cap.draws) continue
            const primary = cap.primary
            out.push(
              item(
                name,
                Kind.Struct,
                primary ? `takes ${primary}` : 'a component this host draws',
                primary ? `${name}($1)` : name,
              ),
            )
          }
        }
        for (const c of names.components) {
          out.push(item(c, Kind.Struct, 'a component this package declares'))
        }
        out.push(item('if', Kind.Keyword, 'one tree or another', 'if ($1) {\n\t$0\n}'))
        out.push(item('for', Kind.Keyword, 'the body once per row', 'for ($1 in $2) {\n\t$0\n}'))
        // And the props this node itself takes.
        const kind = enclosingKind(lines, position.lineNumber - 1)
        const cap = kind ? capability(kind) : undefined
        for (const [name, ty] of Object.entries({ ...(cap?.props ?? {}), ...common() })) {
          out.push(item(name, Kind.Property, ty, `${name}: $0`))
        }
        return { suggestions: out }
      }

      // Inside an action: its phases, and the effects `execute` allows.
      if (block && declaredActions(names).includes(block)) {
        return {
          suggestions: w.phases.map((p) =>
            item(p, Kind.Keyword, 'a phase', `${p} {\n\t$0\n}`),
          ),
        }
      }

      // Anywhere else: the words the language has, and the names this package
      // declares.
      const out = w.keywords.map((k) => item(k, Kind.Keyword, 'a keyword'))
      for (const s of names.state) out.push(item(`state.${s}`, Kind.Variable, 'a state field'))
      for (const f of names.functions) out.push(item(f, Kind.Function, 'a function here'))
      for (const t of [...names.credentials, ...names.types]) {
        out.push(item(t, Kind.Class, 'a type here'))
      }
      for (const e of names.enums) {
        for (const m of e.members) {
          out.push(item(`${e.name}.${m}`, Kind.EnumMember, `a member of ${e.name}`))
        }
      }
      return { suggestions: out }
    },
  })
}

/// The actions a program declares. They arrive beside the names rather than in
/// them, because the analysis already answered that question.
let lastActions: string[] = []
export function rememberActions(actions: string[]) {
  lastActions = actions
}
function declaredActions(_: Declared): string[] {
  return lastActions
}

/// Whether this block is a screen or a component body — the places a component
/// may be written.
function isBody(block: string, names: Declared): boolean {
  return names.screens.includes(block) || names.components.includes(block)
}
