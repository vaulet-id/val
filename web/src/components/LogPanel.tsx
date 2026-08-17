import { cn } from '@/lib/utils'
import { ArrowRight, CircleCheck, CircleSlash, TriangleAlert, XCircle } from 'lucide-react'
import type { RunResult } from '@/val/wasm'

// A press is an action; an action is `(state, input, context, code)` to
// `(state', output, effects)`. That is a reducer, so the panel that reads it
// looks like one: what was dispatched, what changed, and what it asked the host
// to do.
//
// Where it stops looking like a reducer is the part worth reading. A reducer's
// effects happen elsewhere and are somebody else's problem; here they are the
// point, they are requested rather than performed, and the state commits only
// if the host takes them.

const OUTCOME = {
  committed: { icon: CircleCheck, tone: 'text-[var(--color-verified)]', label: 'committed' },
  refused: { icon: CircleSlash, tone: 'text-amber-500', label: 'refused' },
  declined: { icon: CircleSlash, tone: 'text-amber-500', label: 'declined' },
  failed: { icon: TriangleAlert, tone: 'text-amber-500', label: 'ordinary outcome' },
  defect: { icon: XCircle, tone: 'text-red-500', label: 'defect' },
} as const

export type Entry = { at: number; run: RunResult }

export function LogPanel({ entries, onClear }: { entries: Entry[]; onClear: () => void }) {
  if (!entries.length)
    return (
      <p className="p-4 text-[11px] leading-relaxed text-[var(--color-muted-foreground)]">
        Press something in the preview. Every press names an action, so every press
        has a record: what changed, what the host was asked for, and the roots
        before and after.
      </p>
    )

  return (
    <div className="flex flex-col-reverse">
      {entries.map((e) => (
        <LogEntry key={e.at} entry={e} />
      ))}
      <button
        onClick={onClear}
        className="border-b border-[var(--color-border)] px-4 py-1.5 text-left text-[10px] text-[var(--color-muted-foreground)] hover:bg-[var(--color-accent)]"
      >
        clear
      </button>
    </div>
  )
}

function LogEntry({ entry }: { entry: Entry }) {
  const { run } = entry

  if (run.wouldNotBuild)
    return (
      <Frame tone="text-red-500" title="would not build">
        {run.wouldNotBuild.map((e, i) => (
          <div key={i} className="font-mono text-[10px] leading-relaxed">
            {e}
          </div>
        ))}
      </Frame>
    )

  const kind = run.outcome?.kind ?? 'defect'
  const { icon: Icon, tone, label } = OUTCOME[kind]

  return (
    <div className="border-b border-[var(--color-border)] px-4 py-2.5">
      <div className="flex items-baseline gap-2">
        <Icon className={cn('size-3 shrink-0 translate-y-0.5', tone)} />
        <span className="font-mono text-[11px] font-medium">{run.action}</span>
        <span className={cn('text-[10px]', tone)}>{label}</span>
        <span className="ml-auto font-mono text-[9px] text-[var(--color-muted-foreground)]">
          {new Date(entry.at).toLocaleTimeString()}
        </span>
      </div>

      {run.outcome?.why && (
        <p className="mt-1 pl-5 text-[10px] leading-relaxed text-[var(--color-muted-foreground)]">
          {run.outcome.why}
        </p>
      )}

      {!!run.changed?.length && (
        <div className="mt-1.5 pl-5">
          {run.changed.map((c) => (
            <div key={c.path} className="flex items-center gap-1.5 font-mono text-[10px]">
              <span className="w-32 shrink-0 truncate text-[var(--color-muted-foreground)]">{c.path}</span>
              <span className="text-[var(--color-muted-foreground)] line-through">{show(c.from)}</span>
              <ArrowRight className="size-2.5 opacity-50" />
              <span>{show(c.to)}</span>
            </div>
          ))}
        </div>
      )}

      {!!run.effects?.length && (
        <div className="mt-1.5 pl-5">
          <div className="text-[9px] uppercase tracking-widest text-[var(--color-muted-foreground)]">
            asked the host for
          </div>
          {run.effects.map((e, i) => (
            <div key={i} className="font-mono text-[10px]">
              {e.capability}
              {!e.reversible && <span className="ml-1.5 text-amber-500">irreversible</span>}
            </div>
          ))}
        </div>
      )}

      {run.record && (
        <div className="mt-1.5 flex flex-wrap gap-x-3 gap-y-0.5 pl-5 font-mono text-[9px] text-[var(--color-muted-foreground)]">
          <span>root {run.record.previousRoot.slice(0, 8)}</span>
          <ArrowRight className="size-2.5 translate-y-0.5 opacity-50" />
          <span>{run.record.nextRoot.slice(0, 8)}</span>
          <span>· {run.record.bytes} bytes, signed {run.record.signature.slice(0, 8)}</span>
        </div>
      )}
    </div>
  )
}

function Frame({ tone, title, children }: { tone: string; title: string; children: React.ReactNode }) {
  return (
    <div className="border-b border-[var(--color-border)] px-4 py-2.5">
      <div className={cn('text-[11px] font-medium', tone)}>{title}</div>
      <div className="mt-1">{children}</div>
    </div>
  )
}

function show(v: unknown): string {
  if (v === null || v === undefined) return '—'
  if (typeof v === 'object') return JSON.stringify(v)
  return String(v)
}
