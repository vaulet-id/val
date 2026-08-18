import type * as Monaco from 'monaco-editor'

// The shell and the expression layer are highlighted apart, because they are
// read by different people (spec §1, Two readers).
const SHELL = [
  'app', 'version', 'capabilities', 'enum', 'credential', 'type', 'state',
  'trust', 'anchor', 'refines', 'function', 'action', 'screen', 'data',
  'input', 'require', 'verify', 'compute', 'update', 'execute', 'present',
]
const EXPR = ['const', 'if', 'else', 'switch', 'default', 'return', 'with', 'exists', 'from', 'of', 'as', 'order', 'by', 'limit', 'desc', 'asc']
const EFFECT = ['disclose', 'prove', 'navigate']
const TYPES = ['string', 'int', 'bool', 'date', 'datetime', 'bytes', 'List', 'Credential', 'Verified', 'Proof']

export function registerVal(monaco: typeof Monaco) {
  // Registering twice is harmless and simpler than tracking whether the editor
  // or the docs got here first.
  if (monaco.languages.getLanguages().some((l) => l.id === 'val')) return
  monaco.languages.register({ id: 'val' })

  monaco.languages.setMonarchTokensProvider('val', {
    keywords: SHELL,
    expr: EXPR,
    effects: EFFECT,
    types: TYPES,
    tokenizer: {
      root: [
        [/\/\/.*$/, 'comment'],
        [/"[^"]*"/, 'string'],
        [/\b\d[\d_]*\b/, 'number'],
        [/\b\d[\d_]*\.\d+\b/, 'invalid'],
        [
          /[a-zA-Z_][\w]*/,
          {
            cases: {
              '@keywords': 'keyword',
              '@effects': 'type.identifier',
              '@expr': 'keyword.control',
              '@types': 'type',
              '[A-Z][\\w]*': 'constructor',
              '@default': 'identifier',
            },
          },
        ],
        [/[{}()\[\]]/, 'delimiter.bracket'],
        [/[:,.]/, 'delimiter'],
      ],
    },
  })

  monaco.languages.setLanguageConfiguration('val', {
    comments: { lineComment: '//' },
    brackets: [['{', '}'], ['(', ')'], ['[', ']']],
    autoClosingPairs: [
      { open: '{', close: '}' },
      { open: '(', close: ')' },
      { open: '"', close: '"' },
    ],
  })

  for (const [name, base, bg, fg] of [
    ['val-light', 'vs', '#ffffff', '#0a0a0b'],
    ['val-dark', 'vs-dark', '#0a0a0c', '#f2f2f3'],
  ] as const) {
    monaco.editor.defineTheme(name, {
      base: base as Monaco.editor.BuiltinTheme,
      inherit: true,
      rules: [
        { token: 'keyword', foreground: base === 'vs' ? '7c3aed' : 'c4b5fd', fontStyle: 'bold' },
        { token: 'keyword.control', foreground: base === 'vs' ? '2563eb' : '93c5fd' },
        { token: 'type.identifier', foreground: base === 'vs' ? 'b45309' : 'fbbf24' },
        { token: 'type', foreground: base === 'vs' ? '0f766e' : '5eead4' },
        { token: 'constructor', foreground: base === 'vs' ? '0f766e' : '5eead4' },
        { token: 'comment', foreground: base === 'vs' ? '6b7280' : '7d8590', fontStyle: 'italic' },
        { token: 'string', foreground: base === 'vs' ? '15803d' : '86efac' },
        { token: 'number', foreground: base === 'vs' ? 'b91c1c' : 'fca5a5' },
        { token: 'invalid', foreground: 'ef4444', fontStyle: 'underline bold' },
      ],
      colors: { 'editor.background': bg, 'editor.foreground': fg },
    })
  }
}
