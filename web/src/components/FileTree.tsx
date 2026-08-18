import { cn } from '@/lib/utils'
import { ScrollArea } from '@/components/ui/scroll-area'
import { FileCode2, FileJson, FileText, Plus, Trash2 } from 'lucide-react'
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from '@/components/ui/select'
import type { SourceFile } from '@/examples'

type Item = { path: string; name: string; note?: string; pkg?: string }

export function FileTree({
  projects, project, onProject, onNew, onRemove, files, hostFiles, serverFiles, active, onSelect,
}: {
  projects: { id: string; name: string; note: string; builtin: boolean }[]
  project: string
  onProject: (id: string) => void
  onNew: () => void
  onRemove: (id: string) => void
  files: Item[]
  hostFiles?: Item[]
  serverFiles?: Item[]
  active: string
  onSelect: (path: string) => void
}) {
  const here = projects.find((p) => p.id === project)

  return (
    <aside className="flex h-full min-w-0 flex-col">
      {/* One project at a time. Four applications listed together read as one,
          which is how a screen declared in a file you were not looking at ended
          up with no explanation in front of you. */}
      <div className="border-b border-[var(--color-border)] px-2 py-2">
        <div className="flex items-center gap-1">
          <Select value={project} onValueChange={onProject}>
            <SelectTrigger aria-label="project" className="min-w-0 flex-1">
              <SelectValue />
            </SelectTrigger>
            <SelectContent align="start">
              {projects.map((p) => (
                <SelectItem key={p.id} value={p.id}>
                  {p.name}
                </SelectItem>
              ))}
            </SelectContent>
          </Select>
          <button
            onClick={onNew}
            title="a project of your own"
            aria-label="new project"
            className="rounded border border-[var(--color-border)] p-1 hover:bg-[var(--color-accent)]"
          >
            <Plus className="size-3" />
          </button>
          {here && !here.builtin && (
            <button
              onClick={() => onRemove(here.id)}
              title="remove this project"
              aria-label="remove project"
              className="rounded border border-[var(--color-border)] p-1 text-[var(--color-muted-foreground)] hover:bg-[var(--color-accent)]"
            >
              <Trash2 className="size-3" />
            </button>
          )}
        </div>
        {here && (
          <p className="px-0.5 pt-1 text-[10px] leading-snug text-[var(--color-muted-foreground)]">{here.note}</p>
        )}
      </div>

      <Group label="package" files={files} active={active} onSelect={onSelect} />
      {/* The host's own data, in its own group. A `.va` never carries somebody's
          wallet, and putting it under the same heading as the package would be
          the one line of this interface that lied. */}
      {/* The same wallet in every project, because a person has one and every
          application they install looks at that one. */}
      {hostFiles?.length ? <Group label="host" files={hostFiles} active={active} onSelect={onSelect} /> : null}
      {/* And the publisher's own side, which holds their issuer key and never
          goes near the phone. */}
      {serverFiles?.length ? <Group label="server" files={serverFiles} active={active} onSelect={onSelect} /> : null}
      <div className="mt-auto border-t border-[var(--color-border)] px-3 py-2 text-[10px] leading-snug text-[var(--color-muted-foreground)]">
        A package is several files sharing one scope. Nothing is imported across
        one.
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
