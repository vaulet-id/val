import * as React from 'react'
import { cn } from '@/lib/utils'
import { Badge } from '@/components/ui/badge'
import { text as bundle } from '@/examples'
import type { Decl, Node, Program } from '@/val/parse'

// Nothing here executes. There is no evaluator yet, and pretending otherwise
// would be the sort of demo that teaches a language nobody shipped. What this
// draws is the structure the screen declared, with expressions left as the text
// that produced them — which is also honest about who formats values: the host.

function resolve(node: Node, locale: 'th' | 'en') {
  const key = node.args.find((a) => a.name === 'text')?.value.replace(/"/g, '')
  if (!key) return { line: null as React.ReactNode, missing: false }
  const entry = bundle[key]
  if (!entry) return { line: <span className="text-red-500">missing key “{key}”</span>, missing: true }
  const template = entry[locale]
  if (!template) return { line: <span className="text-red-500">“{key}” has no {locale}</span>, missing: true }

  const slots = Object.fromEntries(
    node.args.filter((a) => a.name && a.name !== 'text' && a.name !== 'onTap' && a.name !== 'emphasis').map((a) => [a.name!, a.value]),
  )
  const parts = template.split(/(\{[a-zA-Z_]+\})/g)
  return {
    missing: false,
    line: (
      <>
        {parts.map((part, i) => {
          const m = part.match(/^\{([a-zA-Z_]+)\}$/)
          if (!m) return <span key={i}>{part}</span>
          const expr = slots[m[1]]
          return (
            <span
              key={i}
              title={expr ? `the host formats ${expr} for this locale` : 'no value supplied'}
              className={cn(
                'rounded px-1 font-mono text-[10px]',
                expr ? 'bg-[var(--color-muted)]' : 'bg-red-500/15 text-red-500',
              )}
            >
              {expr ?? `${m[1]}?`}
            </span>
          )
        })}
      </>
    ),
  }
}

function Component({ node, locale, depth = 0 }: { node: Node; locale: 'th' | 'en'; depth?: number }) {
  const [tab, setTab] = React.useState(0)
  const { line } = resolve(node, locale)

  switch (node.kind) {
    case 'column':
      return (
        <div className="flex flex-col gap-2">
          {node.children.map((c, i) => <Component key={i} node={c} locale={locale} depth={depth + 1} />)}
        </div>
      )
    case 'row':
      return (
        <div className="flex items-center justify-between rounded border border-[var(--color-border)] px-2.5 py-2 text-xs">
          {line ?? <span className="text-[var(--color-muted-foreground)]">row</span>}
        </div>
      )
    case 'card':
      return (
        <div className="rounded-lg border border-[var(--color-border)] bg-[var(--color-muted)]/50 px-3 py-3 text-sm">
          {line ?? 'card'}
        </div>
      )
    case 'tabs': {
      const tabs = node.children.filter((c) => c.kind === 'tab')
      return (
        <div className="flex flex-col gap-2">
          <div className="flex gap-1 rounded-md bg-[var(--color-muted)] p-0.5">
            {tabs.map((t, i) => {
              const label = t.args[0]?.value.replace(/"/g, '') ?? `tab ${i}`
              const shown = bundle[label]?.[locale] ?? label
              return (
                <button
                  key={i}
                  onClick={() => setTab(i)}
                  title="which tab is open belongs to the host, never to application state"
                  className={cn(
                    'flex-1 rounded px-2 py-1 text-[11px] transition-colors',
                    tab === i ? 'bg-[var(--color-background)] shadow-sm' : 'text-[var(--color-muted-foreground)]',
                  )}
                >
                  {shown}
                </button>
              )
            })}
          </div>
          <div className="flex flex-col gap-1.5">
            {(tabs[tab]?.children ?? []).map((c, i) => (
              <Component key={i} node={c} locale={locale} depth={depth + 1} />
            ))}
          </div>
        </div>
      )
    }
    case 'list': {
      const child = node.children[0]
      return (
        <div className="flex flex-col gap-1.5">
          {[0, 1, 2].map((i) => (
            <div key={i} className={cn(i === 2 && 'opacity-40')}>
              {child ? (
                <Component node={child} locale={locale} depth={depth + 1} />
              ) : (
                <div className="rounded border border-dashed border-[var(--color-border)] px-2 py-2 text-[11px] text-[var(--color-muted-foreground)]">
                  row
                </div>
              )}
            </div>
          ))}
          <div className="text-[10px] text-[var(--color-muted-foreground)]">
            {node.args[0]?.value ?? '…'} — the host draws the empty state too
          </div>
        </div>
      )
    }
    case 'button': {
      const emphasis = node.args.find((a) => a.name === 'emphasis')?.value.trim()
      const action = node.args.find((a) => a.name === 'onTap')?.value.trim()
      return (
        <button
          title={action ? `calls the action ${action}, through require → verify → compute → update → execute` : undefined}
          className={cn(
            'rounded-md px-3 py-2 text-xs font-medium',
            emphasis === 'primary'
              ? 'bg-[var(--color-primary)] text-[var(--color-primary-foreground)]'
              : 'border border-[var(--color-border)]',
          )}
        >
          {line ?? 'button'}
        </button>
      )
    }
    default:
      return null
  }
}

export function PreviewScreen({ program, locale }: { program: Program; locale: 'th' | 'en' }) {
  const screens = program.decls.filter((d) => d.t === 'screen') as Extract<Decl, { t: 'screen' }>[]

  if (!screens.length)
    return (
      <div className="flex h-full items-center justify-center p-8 text-center text-xs leading-relaxed text-[var(--color-muted-foreground)]">
        This program declares no screen.
        <br />
        An application can be actions, trust policies and state — the loyalty
        card was, before it had one.
      </div>
    )

  return (
    <div className="flex flex-col items-center gap-6 p-5">
      {screens.map((s) => (
        <div key={s.name} className="w-full max-w-[300px]">
          <div className="mb-1.5 flex items-baseline gap-2">
            <span className="font-mono text-[11px] font-medium">{s.name}</span>
            <span className="text-[10px] text-[var(--color-muted-foreground)]">screen</span>
          </div>

          {s.data.length > 0 && (
            <div className="mb-2 flex flex-col gap-1 rounded-md border border-[var(--color-border)] p-2">
              <div className="text-[9px] font-semibold uppercase tracking-widest text-[var(--color-muted-foreground)]">
                what this screen sees
              </div>
              {s.data.map((d) => (
                <div key={d.name} className="flex items-center gap-1.5 font-mono text-[10px]">
                  <Badge variant={d.source === 'credentials' ? (d.policy ? 'verified' : 'default') : 'origin'}>
                    {d.source === 'credentials' ? (d.policy ? 'issuer' : 'unverified') : 'origin'}
                  </Badge>
                  <span>{d.name}</span>
                  <span className="truncate text-[var(--color-muted-foreground)]">
                    {d.credentialType ?? d.audience}
                    {d.policy ? ` · ${d.policy}` : ''}
                  </span>
                </div>
              ))}
            </div>
          )}

          {/* The frame is the host's. Everything inside it was declared, and
              nothing inside it was drawn by the application. */}
          <div className="rounded-[1.6rem] border-[6px] border-[var(--color-foreground)]/85 bg-[var(--color-background)] p-3 shadow-lg">
            <div className="mx-auto mb-2 h-1 w-10 rounded-full bg-[var(--color-foreground)]/20" />
            <div className="flex flex-col gap-2">
              {s.tree.map((n, i) => <Component key={i} node={n} locale={locale} />)}
            </div>
          </div>

          {s.compute.length > 0 && (
            <div className="mt-2 rounded-md border border-dashed border-[var(--color-border)] p-2">
              <div className="mb-1 text-[9px] font-semibold uppercase tracking-widest text-[var(--color-muted-foreground)]">
                derived, never persisted
              </div>
              {s.compute.map((c, i) => (
                <div key={i} className="truncate font-mono text-[10px] text-[var(--color-muted-foreground)]">{c}</div>
              ))}
            </div>
          )}
        </div>
      ))}
    </div>
  )
}
