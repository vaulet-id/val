import { Badge } from '@/components/ui/badge'
import { cn } from '@/lib/utils'
import { AlertTriangle, XCircle } from 'lucide-react'
import type { Report } from '@/val/report'

export function ReportPanel({ report }: { report: Report }) {
  return (
    <div className="flex flex-col gap-4 p-4 font-mono text-[11px]">
      <div>
        <div className="mb-1 text-[9px] font-semibold uppercase tracking-widest text-[var(--color-muted-foreground)]">
          capability report
        </div>
        <div className="text-xs">{report.app} <span className="text-[var(--color-muted-foreground)]">v{report.version}</span></div>
        <p className="mt-1 max-w-prose font-sans text-[10px] leading-relaxed text-[var(--color-muted-foreground)]">
          Derived from the code, not declared by its author — which is why a
          publisher cannot understate it. The host recomputes this and refuses on
          mismatch, and the consent sheet is a rendering of it.
        </p>
      </div>

      <table className="w-full border-collapse">
        <tbody>
          {report.lines.map((l) => (
            <tr key={l.label} className="border-t border-[var(--color-border)] align-top">
              <td className="w-28 py-1.5 pr-3 text-[var(--color-muted-foreground)]">{l.label}</td>
              <td className={cn('py-1.5', l.tone === 'warn' && l.values.length && 'text-amber-500')}>
                {l.values.length ? (
                  l.values.map((v, i) => <div key={i} className="break-words">{v}</div>)
                ) : (
                  <span className="text-[var(--color-muted-foreground)]">—</span>
                )}
              </td>
            </tr>
          ))}
        </tbody>
      </table>

      {report.findings.length > 0 && (
        <div className="flex flex-col gap-2">
          <div className="text-[9px] font-semibold uppercase tracking-widest text-[var(--color-muted-foreground)]">
            would not build
          </div>
          {report.findings.map((f, i) => (
            <div key={i} className="flex gap-2 rounded border border-[var(--color-border)] p-2">
              {f.severity === 'error' ? (
                <XCircle className="mt-0.5 size-3 shrink-0 text-red-500" />
              ) : (
                <AlertTriangle className="mt-0.5 size-3 shrink-0 text-amber-500" />
              )}
              <div className="font-sans text-[11px] leading-relaxed">
                {f.line > 0 && <span className="mr-1 font-mono text-[10px] text-[var(--color-muted-foreground)]">line {f.line}</span>}
                {f.message}
              </div>
            </div>
          ))}
        </div>
      )}

      <div className="mt-2 flex flex-wrap items-center gap-1.5 border-t border-[var(--color-border)] pt-3">
        <span className="font-sans text-[10px] text-[var(--color-muted-foreground)]">three grades of data:</span>
        <Badge variant="verified">issuer-backed</Badge>
        <Badge variant="self">self-asserted</Badge>
        <Badge variant="origin">origin-asserted</Badge>
      </div>
    </div>
  )
}
