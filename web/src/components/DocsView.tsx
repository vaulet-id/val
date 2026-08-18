import * as React from 'react'
import { useMonaco } from '@monaco-editor/react'
import { cn } from '@/lib/utils'
import { ScrollArea } from '@/components/ui/scroll-area'
import { ChevronRight } from 'lucide-react'
import { groupsFor, type Locale, pagesFor, render } from '@/docs'
import { registerVal } from '@/val/monaco-lang'

// Documentation, laid out the way documentation is laid out: pages down the
// left, the page in the middle, and where you are inside it down the right.
//
// The middle column is measured rather than filled — a specification is read in
// long paragraphs, and a line of prose that runs the width of a monitor is one
// the eye loses on the way back.

export function DocsView({ dark, locale }: { dark: boolean; locale: Locale }) {
  const docs = React.useMemo(() => pagesFor(locale), [locale])
  const groups = React.useMemo(() => groupsFor(locale), [locale])
  const [active, setActive] = React.useState(docs[0].id)
  const [seen, setSeen] = React.useState<string | null>(null)
  const body = React.useRef<HTMLDivElement>(null)
  const monaco = useMonaco()

  // The pages are the same pages in either language, so switching language
  // keeps the reader's place by position — the ids are translated titles and
  // would otherwise send them back to page one.
  const at = React.useRef(0)
  const found = docs.findIndex((d) => d.id === active)
  if (found >= 0) at.current = found
  const page = docs[found >= 0 ? found : at.current] ?? docs[0]
  React.useEffect(() => {
    if (page.id !== active) setActive(page.id)
  }, [page, active])
  const html = React.useMemo(() => render(page.markdown), [page])

  // The same highlighter as the editor, over the same grammar. A second one
  // would be a second answer to "is this a keyword" — and the two would
  // eventually disagree in a document that is teaching the language.
  React.useEffect(() => {
    const root = body.current
    if (!monaco || !root) return
    registerVal(monaco)
    let cancelled = false

    for (const block of root.querySelectorAll<HTMLElement>('figure.code')) {
      const code = block.querySelector('code')
      const language = block.querySelector('figcaption')?.textContent?.trim() ?? 'val'
      if (!code || code.dataset.lit) continue
      monaco.editor.colorize(code.textContent ?? '', language, { tabSize: 2 }).then((html: string) => {
        if (cancelled) return
        code.innerHTML = html
        code.dataset.lit = '1'
      })
    }
    return () => {
      cancelled = true
    }
  }, [monaco, html, dark])

  // Which heading you are under, from the one nearest the top of the viewport.
  React.useEffect(() => {
    const root = body.current
    if (!root) return
    const marks = [...root.querySelectorAll<HTMLElement>('h3[id]')]
    const onScroll = () => {
      const top = root.getBoundingClientRect().top + 80
      const current = marks.filter((m) => m.getBoundingClientRect().top <= top).pop()
      setSeen(current?.id ?? null)
    }
    onScroll()
    const scroller = root.closest('[data-radix-scroll-area-viewport]')
    scroller?.addEventListener('scroll', onScroll)
    return () => scroller?.removeEventListener('scroll', onScroll)
  }, [html])

  const go = (id: string) => {
    setActive(id)
    setSeen(null)
    body.current?.closest('[data-radix-scroll-area-viewport]')?.scrollTo({ top: 0 })
  }

  return (
    <div className="flex min-h-0 flex-1">
      {/* Pages */}
      <nav className="w-64 shrink-0 overflow-hidden border-r border-[var(--color-border)]">
        <ScrollArea className="h-full">
          <div className="px-3 py-4">
            {groups.map((group) => (
              <div key={group} className="mb-5">
                <div className="px-2 pb-1.5 text-[10px] font-semibold uppercase tracking-widest text-[var(--color-muted-foreground)]">
                  {group}
                </div>
                {docs
                  .filter((d) => d.group === group)
                  .map((d) => (
                    <button
                      key={d.id}
                      onClick={() => go(d.id)}
                      className={cn(
                        'block w-full rounded px-2 py-1 text-left text-[13px] leading-snug transition-colors',
                        d.id === active
                          ? 'bg-[var(--color-accent)] font-medium'
                          : 'text-[var(--color-muted-foreground)] hover:text-[var(--color-foreground)]',
                      )}
                    >
                      {d.title}
                    </button>
                  ))}
              </div>
            ))}
          </div>
        </ScrollArea>
      </nav>

      {/* The page */}
      <ScrollArea className="min-w-0 flex-1">
        <div ref={body} className="mx-auto max-w-[46rem] px-10 py-8">
          <div className="flex items-center gap-1.5 text-[12px] text-[var(--color-muted-foreground)]">
            <span>VAL</span>
            <ChevronRight className="size-3" />
            <span>{page.section}</span>
            <ChevronRight className="size-3" />
            <span className="text-[var(--color-foreground)]">{page.title}</span>
          </div>

          <h1 className="mt-3 text-[2rem] font-bold leading-tight tracking-[-0.02em]">{page.title}</h1>
          {page.lede && (
            <p className="mt-2 text-[15px] leading-relaxed text-[var(--color-muted-foreground)]">{page.lede}</p>
          )}
          <hr className="mt-6 mb-8 border-[var(--color-border)]" />

          <article className="prose-val" dangerouslySetInnerHTML={{ __html: html }} />
        </div>
      </ScrollArea>

      {/* Where you are inside it */}
      <aside className="hidden w-60 shrink-0 border-l border-[var(--color-border)] xl:block">
        <ScrollArea className="h-full">
          <div className="px-5 py-8">
            <div className="pb-2 text-[13px] font-semibold">On this page</div>
            {page.headings.length === 0 ? (
              <p className="text-[11px] leading-relaxed text-[var(--color-muted-foreground)]">
                One thing, said once.
              </p>
            ) : (
              page.headings.map((h) => (
                <a
                  key={h.id}
                  href={`#${h.id}`}
                  onClick={(e) => {
                    e.preventDefault()
                    body.current?.querySelector(`#${CSS.escape(h.id)}`)?.scrollIntoView({ block: 'start' })
                  }}
                  className={cn(
                    'block py-1 text-[12px] leading-snug transition-colors',
                    seen === h.id
                      ? 'font-medium text-[var(--color-origin)]'
                      : 'text-[var(--color-muted-foreground)] hover:text-[var(--color-foreground)]',
                  )}
                >
                  {h.text}
                </a>
              ))
            )}
          </div>
        </ScrollArea>
      </aside>
    </div>
  )
}
