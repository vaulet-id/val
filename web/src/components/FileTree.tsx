import { cn } from '@/lib/utils'
import { ScrollArea } from '@/components/ui/scroll-area'
import { FileCode2, FileJson, FileText } from 'lucide-react'
import type { SourceFile } from '@/examples'

export function FileTree({
  files, active, onSelect, heading,
}: {
  files: { path: string; name: string; note?: string }[]
  active: string
  onSelect: (path: string) => void
  heading: string
}) {
  return (
    <aside className="flex h-full min-w-0 flex-col">
      <div className="px-3 py-2 text-[10px] font-semibold uppercase tracking-widest text-[var(--color-muted-foreground)]">
        {heading}
      </div>
      <ScrollArea className="flex-1">
        <nav className="px-1.5 pb-3">
          {files.map((f) => {
            const Icon = f.name.endsWith('.json') ? FileJson : f.name.endsWith('.val') ? FileCode2 : FileText
            return (
              <button
                key={f.path}
                onClick={() => onSelect(f.path)}
                className={cn(
                  'flex w-full flex-col gap-0.5 rounded px-2 py-1.5 text-left transition-colors',
                  active === f.path ? 'bg-[var(--color-accent)]' : 'hover:bg-[var(--color-accent)]/60',
                )}
              >
                <span className="flex items-center gap-1.5 font-mono text-xs">
                  <Icon className="size-3 shrink-0 opacity-60" />
                  {f.name}
                </span>
                {f.note && (
                  <span className="pl-[18px] text-[10px] leading-tight text-[var(--color-muted-foreground)]">
                    {f.note}
                  </span>
                )}
              </button>
            )
          })}
        </nav>
      </ScrollArea>
      <div className="border-t border-[var(--color-border)] px-3 py-2 text-[10px] leading-snug text-[var(--color-muted-foreground)]">
        One package, several files, one scope — no imports across packages.
      </div>
    </aside>
  )
}

export type { SourceFile }
