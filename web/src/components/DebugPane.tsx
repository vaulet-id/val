import * as React from 'react'
import { cn } from '@/lib/utils'
import { Tabs, TabsContent, TabsList, TabsTrigger } from '@/components/ui/tabs'
import { ScrollArea } from '@/components/ui/scroll-area'
import { LogPanel, type Entry } from '@/components/LogPanel'
import { X, TriangleAlert, XCircle } from 'lucide-react'
import type { Diagnostic } from '@/val/wasm'

// The pane under the editor, where an editor keeps this sort of thing.
//
// Problems and the log belong together and belong here: both are about the
// program in front of you, and both were on the far side of the window from it.

export function DebugPane({
  diagnostics, entries, onClear, onClose, onGo,
}: {
  diagnostics: Diagnostic[]
  entries: Entry[]
  onClear: () => void
  onClose: () => void
  onGo: (line: number) => void
}) {
  const [tab, setTab] = React.useState('problems')

  // A press is worth looking at the moment it happens, so the pane follows it.
  React.useEffect(() => {
    if (entries.length) setTab('log')
  }, [entries.length])

  return (
    <Tabs value={tab} onValueChange={setTab} className="flex h-full min-h-0 flex-col">
      <div className="flex shrink-0 items-center gap-2 border-y border-[var(--color-border)] px-2 py-2">
        <TabsList>
          <TabsTrigger value="problems">
            Problems{diagnostics.length ? ` ${diagnostics.length}` : ''}
          </TabsTrigger>
          <TabsTrigger value="log">Log{entries.length ? ` ${entries.length}` : ''}</TabsTrigger>
        </TabsList>
        <button
          onClick={onClose}
          className="ml-auto rounded p-1 text-[var(--color-muted-foreground)] hover:bg-[var(--color-accent)]"
          aria-label="close the pane"
        >
          <X className="size-3" />
        </button>
      </div>

      <TabsContent value="problems" className="min-h-0 flex-1">
        <ScrollArea className="h-full">
          {diagnostics.length === 0 ? (
            <p className="p-3 text-[11px] text-[var(--color-muted-foreground)]">
              Nothing to say about this package.
            </p>
          ) : (
            diagnostics.map((d, i) => (
              <button
                key={i}
                onClick={() => onGo(d.line)}
                className="flex w-full gap-2 border-b border-[var(--color-border)]/60 px-3 py-1.5 text-left hover:bg-[var(--color-accent)]"
              >
                {d.severity === 'error' ? (
                  <XCircle className="mt-0.5 size-3 shrink-0 text-red-500" />
                ) : (
                  <TriangleAlert className="mt-0.5 size-3 shrink-0 text-amber-500" />
                )}
                <span className={cn('font-mono text-[10px] text-[var(--color-muted-foreground)]')}>
                  {d.line}:{d.column}
                </span>
                <span className="min-w-0 flex-1 text-[11px] leading-relaxed">{d.message}</span>
              </button>
            ))
          )}
        </ScrollArea>
      </TabsContent>

      <TabsContent value="log" className="min-h-0 flex-1">
        <ScrollArea className="h-full">
          <LogPanel entries={entries} onClear={onClear} />
        </ScrollArea>
      </TabsContent>
    </Tabs>
  )
}
