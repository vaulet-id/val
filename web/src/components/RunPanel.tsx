import { cn } from '@/lib/utils'
import { Badge } from '@/components/ui/badge'
import { Check, X, AlertTriangle, ShieldQuestion } from 'lucide-react'
import type { BuildResult, RunResult } from '@/val/run'

const OUTCOME: Record<RunResult['outcome'], { label: string; tone: string; blurb: string }> = {
  committed: { label: 'would commit', tone: 'text-[var(--color-verified)]', blurb: 'the host was offered the batch; if it takes all of it, the next state commits' },
  refused:   { label: 'refused', tone: 'text-amber-500', blurb: 'the host declined the batch, so nothing commits' },
  defect:    { label: 'defect', tone: 'text-red-500', blurb: 'a `require` or a trap: the application asked for something it had no business asking' },
  outcome:   { label: 'ordinary outcome', tone: 'text-amber-500', blurb: '`verify` failed — the person is told, and the application did nothing wrong' },
}

export function RunPanel({ build, result }: { build: BuildResult; result: RunResult | null }) {
  return (
    <div className="flex flex-col gap-4 p-4 text-[11px]">
      <section>
        <Head>build</Head>
        <div className="flex flex-col gap-1">
          {build.checks.map((c) => (
            <div key={c.name} className="flex items-center gap-2 font-mono">
              {c.ok ? <Check className="size-3 text-[var(--color-verified)]" /> : <X className="size-3 text-red-500" />}
              <span className="w-24">{c.name}</span>
              <span className="text-[var(--color-muted-foreground)]">{c.note}</span>
            </div>
          ))}
        </div>
        {build.problems.length > 0 && (
          <div className="mt-2 flex flex-col gap-1.5">
            {build.problems.map((p, i) => (
              <div key={i} className="flex gap-2 rounded border border-[var(--color-border)] p-2 leading-relaxed">
                <AlertTriangle className={cn('mt-0.5 size-3 shrink-0', p.severity === 'error' ? 'text-red-500' : 'text-amber-500')} />
                <span>
                  {p.line > 0 && <span className="mr-1 font-mono text-[10px] text-[var(--color-muted-foreground)]">line {p.line}</span>}
                  {p.message}
                </span>
              </div>
            ))}
          </div>
        )}
      </section>

      {!result ? (
        <p className="text-[var(--color-muted-foreground)]">
          {build.ok ? 'This program declares no action to run.' : 'Fix the build first — the host runs the same checks and would refuse this package.'}
        </p>
      ) : (
        <>
          <section>
            <Head>run · {result.action}</Head>
            <div className={cn('font-mono text-xs', OUTCOME[result.outcome].tone)}>{OUTCOME[result.outcome].label}</div>
            <p className="mt-1 leading-relaxed text-[var(--color-muted-foreground)]">
              {result.message ?? OUTCOME[result.outcome].blurb}
            </p>
          </section>

          <section>
            <Head>phases</Head>
            <div className="flex flex-col gap-2">
              {result.trace.map((t) => (
                <div key={t.phase}>
                  <div className="font-mono text-[10px] uppercase tracking-widest text-[var(--color-muted-foreground)]">{t.phase}</div>
                  {t.lines.map((l, i) => (
                    <div key={i} className="flex items-baseline gap-2 border-b border-[var(--color-border)]/60 py-0.5">
                      <span className="min-w-0 flex-1 truncate font-mono text-[10px]" title={l.text}>{l.text}</span>
                      {l.value && (
                        <span className={cn('shrink-0 font-mono text-[10px]', l.value.startsWith('trap') ? 'text-red-500' : 'text-[var(--color-muted-foreground)]')}>
                          {l.value}
                        </span>
                      )}
                    </div>
                  ))}
                </div>
              ))}
            </div>
          </section>

          {result.effects.length > 0 && (
            <section>
              <Head>the batch the host is offered</Head>
              {/* Requested, never performed. All of them or none, and the state
                  commits only if the host took them — spec §5. */}
              <div className="rounded-lg border border-[var(--color-border)] p-2.5">
                <div className="mb-1.5 flex items-center gap-1.5 text-[10px] text-[var(--color-muted-foreground)]">
                  <ShieldQuestion className="size-3" />
                  host chrome — the application cannot draw or cover this
                </div>
                {result.effects.map((e, i) => (
                  <div key={i} className="flex flex-col gap-0.5 border-t border-[var(--color-border)] py-1.5 first:border-t-0">
                    <span className="font-mono text-[10px]">{e.capability}</span>
                    <span className="break-words font-mono text-[10px] text-[var(--color-muted-foreground)]">{e.payload}</span>
                  </div>
                ))}
                <div className="mt-1 text-[10px] text-[var(--color-muted-foreground)]">all of them, or none</div>
              </div>
            </section>
          )}

          <section>
            <Head>execution record</Head>
            <table className="w-full font-mono text-[10px]">
              <tbody>
                <Row k="action" v={result.action} />
                <Row k="context.time" v={result.context.time} />
                <Row k="context.uuid" v={result.context.uuid} />
                <Row k="previous root" v={result.prevRoot.slice(0, 24) + '…'} />
                <Row k="next root" v={result.nextRoot.slice(0, 24) + '…'} />
                <Row k="effects" v={String(result.effects.length)} />
              </tbody>
            </table>
            <p className="mt-1.5 leading-relaxed text-[10px] text-[var(--color-muted-foreground)]">
              The next root is the Merkle root over the state's <code>(path, value)</code> leaves,
              so one field can be proved without opening the rest. A verifier that
              remembers the previous root is what catches a rollback.
            </p>
          </section>

          <section>
            <Head>state leaves</Head>
            <div className="flex flex-col">
              {result.leaves.map((l) => (
                <div key={l.path} className="flex items-baseline gap-2 border-b border-[var(--color-border)]/60 py-0.5 font-mono text-[10px]">
                  <span className="w-32 shrink-0 truncate">{l.path}</span>
                  <span className="flex-1 truncate">{l.value}</span>
                  <span className="shrink-0 text-[var(--color-muted-foreground)]">{l.hash.slice(0, 8)}</span>
                </div>
              ))}
            </div>
          </section>

          <div className="flex items-center gap-1.5 border-t border-[var(--color-border)] pt-3">
            <Badge>simulated</Badge>
            <span className="leading-snug text-[10px] text-[var(--color-muted-foreground)]">
              No evaluator exists yet. Credentials here are shaped like their
              declarations and signed by nobody; the canonical encoding stands in
              for dCBOR. The shape is right and the bytes are not.
            </span>
          </div>
        </>
      )}
    </div>
  )
}

const Head = ({ children }: { children: React.ReactNode }) => (
  <div className="mb-1.5 text-[9px] font-semibold uppercase tracking-widest text-[var(--color-muted-foreground)]">{children}</div>
)

const Row = ({ k, v }: { k: string; v: string }) => (
  <tr className="border-t border-[var(--color-border)]">
    <td className="w-28 py-1 pr-2 text-[var(--color-muted-foreground)]">{k}</td>
    <td className="break-all py-1">{v}</td>
  </tr>
)
