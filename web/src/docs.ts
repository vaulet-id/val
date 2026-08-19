import { marked } from 'marked'
import spec from '../../docs/spec.md?raw'
import specTh from '../../docs/th/spec.md?raw'

// The guide, which is the documentation somebody arrives for: they want to
// build a Micro App, not to argue about the language. The specification stays,
// at the bottom, because a guide that answers most questions still has to
// point at the thing that answers the rest exactly.
import g00 from '../../docs/guide/00-why-a-new-language.md?raw'
import g01 from '../../docs/guide/01-what-you-are-building.md?raw'
import g02 from '../../docs/guide/02-your-first-application.md?raw'
import g03 from '../../docs/guide/03-capabilities.md?raw'
import g04 from '../../docs/guide/04-credentials-and-trust.md?raw'
import g05 from '../../docs/guide/05-actions.md?raw'
import g06 from '../../docs/guide/06-screens.md?raw'
import g07 from '../../docs/guide/07-disclosing-and-proving.md?raw'
import g08 from '../../docs/guide/08-state-and-versions.md?raw'
import g09 from '../../docs/guide/09-publishing.md?raw'
import g10 from '../../docs/guide/10-reference.md?raw'

// The same guide in Thai. A translation and not a second document: when the
// English page changes, this one is wrong until it changes too, which is a
// smaller problem than two guides that disagree about the language.
import t00 from '../../docs/guide/th/00-why-a-new-language.md?raw'
import t01 from '../../docs/guide/th/01-what-you-are-building.md?raw'
import t02 from '../../docs/guide/th/02-your-first-application.md?raw'
import t03 from '../../docs/guide/th/03-capabilities.md?raw'
import t04 from '../../docs/guide/th/04-credentials-and-trust.md?raw'
import t05 from '../../docs/guide/th/05-actions.md?raw'
import t06 from '../../docs/guide/th/06-screens.md?raw'
import t07 from '../../docs/guide/th/07-disclosing-and-proving.md?raw'
import t08 from '../../docs/guide/th/08-state-and-versions.md?raw'
import t09 from '../../docs/guide/th/09-publishing.md?raw'
import t10 from '../../docs/guide/th/10-reference.md?raw'

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

/// A title, as an id.
///
/// Letters and numbers in any script, not `a-z0-9`: every Thai title stripped
/// to the empty string, so every page in the Thai guide had the same id — the
/// nav highlighted all of them at once and opening one opened the first.
const slug = (s: string) =>
  s
    .toLowerCase()
    .replace(/[^\p{L}\p{N}\s-]/gu, '')
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

/// A guide page is a whole document — its `#` title, its own sections. Cutting
/// one at `##` would scatter a single explanation across five nav entries.
function page(markdown: string, group: string): Page {
  const lines = markdown.split('\n')
  const title = (lines.find((l) => l.startsWith('# ')) ?? '# Untitled').slice(2).trim()
  const body = lines.filter((l) => !l.startsWith('# ')).join('\n').trim()
  const headings = body
    .split('\n')
    .filter((l) => l.startsWith('## '))
    .map((l) => {
      const text = l.slice(3).replace(/[`*]/g, '')
      return { id: slug(text), text }
    })
  const lede = body
    .split('\n\n')
    .map((p) => p.trim())
    .find((p) => p && !p.startsWith('#') && !p.startsWith('```') && !p.startsWith('|') && !p.startsWith('>'))
  const lifted = lede && lede.length < 320 ? lede : undefined
  return {
    id: `${slug(group)}/${slug(title)}`,
    group,
    title,
    section: group,
    lede: lifted?.replace(/[`*]/g, ''),
    markdown: lifted ? body.replace(lifted, '').trim() : body,
    headings,
  }
}

/// Which group a specification section belongs under.
///
/// Twelve numbered sections in a flat list is a table of contents, not a
/// reading order — the numbers say what comes next and nothing about what
/// belongs together. These say what a reader is doing when they open one:
/// finding out what this is, learning to write it, understanding what happens
/// when it runs, or looking for the parts nobody has settled.
const SPEC_GROUPS: [string, string[]][] = [
  ['The specification', []],
]

function grouped(markdown: string, fallback: string): Page[] {
  return pages(markdown, fallback, 'Overview').map((p) => {
    const group = SPEC_GROUPS.find(([, titles]) => titles.includes(p.title))?.[0]
    return group ? { ...p, group, section: group, id: `${slug(group)}/${slug(p.title)}` } : p
  })
}

/// The groups, in each language. Nav headings a person cannot read are a
/// translation that stopped halfway down the page.
const GROUPS = {
  en: { start: 'Getting started', build: 'Building an app', ship: 'Shipping it', spec: 'The specification' },
  th: { start: 'เริ่มต้น', build: 'สร้างแอป', ship: 'ปล่อยของ', spec: 'ข้อกำหนดภาษา' },
} as const

export type Locale = keyof typeof GROUPS

/// The specification is translated too, and its Thai opens by saying the
/// English is what a disagreement is settled against. A translation that says
/// which one is normative has one answer the day it drifts; two documents that
/// do not have two.
export function pagesFor(locale: Locale): Page[] {
  const g = GROUPS[locale]
  const guide = locale === 'th'
    ? [t00, t01, t02, t03, t04, t05, t06, t07, t08, t09, t10]
    : [g00, g01, g02, g03, g04, g05, g06, g07, g08, g09, g10]
  // First, because it answers the question somebody has before they have
  // decided to read any of the rest.
  const where = [g.start, g.start, g.start, g.build, g.build, g.build, g.build, g.build, g.build, g.ship, g.ship]
  return [
    ...guide.map((md, i) => page(md, where[i])),
    ...grouped(locale === 'th' ? specTh : spec, g.spec).map((p) => ({ ...p, group: g.spec, section: g.spec })),
  ]
}

export function groupsFor(locale: Locale): string[] {
  const g = GROUPS[locale]
  const order = [g.start, g.build, g.ship, g.spec]
  const all = pagesFor(locale)
  return [
    ...order.filter((name) => all.some((d) => d.group === name)),
    ...[...new Set(all.map((d) => d.group))].filter((name) => !order.includes(name as never)),
  ]
}

export const docs: Page[] = pagesFor('en')

export const groups = groupsFor('en')

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
