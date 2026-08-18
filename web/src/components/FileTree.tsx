import { cn } from '@/lib/utils'
import { ScrollArea } from '@/components/ui/scroll-area'
import { FileCode2, FileJson, FileText } from 'lucide-react'
import type { SourceFile } from '@/examples'

type Item = { path: string; name: string; note?: string }

export function FileTree({
  files, hostFiles, serverFiles, active, onSelect, heading,
}: {
  files: Item[]
  hostFiles?: Item[]
  serverFiles?: Item[]
  active: string
  onSelect: (path: string) => void
  heading: string
}) {
  return (
    <aside className="flex h-full min-w-0 flex-col">
      <Group label={heading} files={files} active={active} onSelect={onSelect} />
      {/* The host's own data, in its own group. A `.va` never carries somebody's
          wallet, and putting it under the same heading as the package would be
          the one line of this interface that lied. */}
      {hostFiles?.length ? <Group label="host" files={hostFiles} active={active} onSelect={onSelect} /> : null}
      {/* And the publisher's own side, which holds their issuer key and never
          goes near the phone. */}
      {serverFiles?.length ? <Group label="server" files={serverFiles} active={active} onSelect={onSelect} /> : null}
      <div className="mt-auto border-t border-[var(--color-border)] px-3 py-2 text-[10px] leading-snug text-[var(--color-muted-foreground)]">
        One package, several files, one scope — no imports across packages.
      </div>
    </aside>
  )
}

function Group({
  label, files, active, onSelect,
}: {
  label: string
  files: Item[]
  active: string
  onSelect: (path: string) => void
}) {
  return (
    <>
      <div className="px-3 py-2 text-[10px] font-semibold uppercase tracking-widest text-[var(--color-muted-foreground)]">
        {label}
      </div>
      <ScrollArea className="shrink-0">
        <nav className="px-1.5 pb-2">
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
    </>
  )
}

export type { SourceFile }
