import { marked } from 'marked'
import spec from '../../docs/spec.md?raw'
import readme from '../../README.md?raw'
import examples from '../../examples/README.md?raw'
import preview from '../../preview/README.md?raw'

// The documents, cut into pages at their `##` headings.
//
// One file scrolled for two thousand lines is a file nobody finishes. The
// specification is already written in sections that each answer one question,
// so the sections are the pages — and the cut is made here rather than in the
// source, because the source is the thing a reader clones and greps.

export type Page = {
  id: string
  group: string
  /// What the left nav shows: `5. Actions` becomes `Actions`, numbered by its
  /// position, because a number in a link is a number that goes stale.
  title: string
  /// The breadcrumb's last-but-one element.
  section: string
  lede?: string
  markdown: string
  headings: { id: string; text: string }[]
}

const slug = (s: string) =>
  s
    .toLowerCase()
    .replace(/[^a-z0-9\s-]/g, '')
    .trim()
    .replace(/\s+/g, '-')

/// Split one document at its `##` headings. Anything before the first one is
/// the document's own opening, which becomes a page in its own right — it is
/// usually the part that says what the rest is for.
function pages(markdown: string, group: string, openingTitle: string): Page[] {
  const lines = markdown.split('\n')
  const out: Page[] = []
  let title = openingTitle
  let buffer: string[] = []

  const flush = () => {
    const body = buffer.join('\n').trim()
    if (!body && out.length) return
    const headings = body
      .split('\n')
      .filter((l) => l.startsWith('### '))
      .map((l) => {
        const text = l.slice(4).replace(/[`*]/g, '')
        return { id: slug(text), text }
      })
    // The first paragraph, when it reads like a summary rather than a sentence
    // that needs the one before it.
    const lede = body
      .split('\n\n')
      .map((p) => p.trim())
      .find((p) => p && !p.startsWith('#') && !p.startsWith('```') && !p.startsWith('|') && !p.startsWith('>'))
    // A lede lifted out of the body has to leave the body, or the page opens by
    // saying the same thing twice.
    const lifted = lede && lede.length < 320 ? lede : undefined
    out.push({
      id: `${slug(group)}/${slug(title)}`,
      group,
      title: title.replace(/^\d+\.\s*/, ''),
      section: group,
      lede: lifted?.replace(/[`*]/g, ''),
      markdown: lifted ? body.replace(lifted, '').trim() : body,
      headings,
    })
    buffer = []
  }

  for (const line of lines) {
    if (line.startsWith('## ')) {
      flush()
      title = line.slice(3).trim()
      continue
    }
    if (line.startsWith('# ')) continue
    buffer.push(line)
  }
  flush()
  return out
}

/// Which group a specification section belongs under.
///
/// Twelve numbered sections in a flat list is a table of contents, not a
/// reading order — the numbers say what comes next and nothing about what
/// belongs together. These say what a reader is doing when they open one:
/// finding out what this is, learning to write it, understanding what happens
/// when it runs, or looking for the parts nobody has settled.
const SPEC_GROUPS: [string, string[]][] = [
  ['Start here', ['What VAL is for', 'Shape of a program']],
  ['Writing it', ['Values and types', 'Verification', 'Actions', 'Totality']],
  ['Running it', ['Execution and its record', 'Compilation target', 'Proofs']],
  ['Interfaces', ['User interface']],
  ['Unsettled', ['Open questions', 'Order of work']],
]

function grouped(markdown: string, fallback: string): Page[] {
  return pages(markdown, fallback, 'Overview').map((p) => {
    const group = SPEC_GROUPS.find(([, titles]) => titles.includes(p.title))?.[0]
    return group ? { ...p, group, section: group, id: `${slug(group)}/${slug(p.title)}` } : p
  })
}

export const docs: Page[] = [
  ...grouped(spec, 'Start here'),
  ...pages(readme, 'Why VAL', 'Overview'),
  ...pages(examples, 'Examples', 'Overview'),
  ...pages(preview, 'The renderer', 'Overview'),
]

/// In the order the groups are written above, then anything else — a group that
/// arrives later should appear where it belongs rather than where it was added.
export const groups = [
  ...SPEC_GROUPS.map(([name]) => name).filter((g) => docs.some((d) => d.group === g)),
  ...[...new Set(docs.map((d) => d.group))].filter((g) => !SPEC_GROUPS.some(([name]) => name === g)),
]

marked.use({
  renderer: {
    heading({ text, depth }) {
      const clean = text.replace(/<[^>]+>/g, '')
      const id = slug(clean)
      return `<h${depth} id="${id}">${text}</h${depth}>\n`
    },
    code({ text, lang }) {
      const escaped = text.replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;')
      // The language chip, which is the one piece of chrome a code block earns:
      // it says what you are looking at before you have read a line of it.
      return `<figure class="code"><figcaption>${lang || 'val'}</figcaption><pre><code>${escaped}</code></pre></figure>\n`
    },
  },
})

export const render = (markdown: string) => marked.parse(markdown) as string
