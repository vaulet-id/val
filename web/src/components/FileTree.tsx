import { cn } from '@/lib/utils'
import { ScrollArea } from '@/components/ui/scroll-area'
import { FileCode2, FileJson, FileText, Plus, Trash2 } from 'lucide-react'
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from '@/components/ui/select'
import type { Group as GroupName, SourceFile } from '@/examples'

type Item = { path: string; name: string; note?: string; pkg?: string }

export function FileTree({
  projects, project, onProject, onNew, onRemove, files, hostFiles, serverFiles, active, onSelect,
  onAddFile, onRemoveFile, canRemove,
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
  onAddFile: (group: GroupName) => void
  onRemoveFile: (path: string) => void
  canRemove: (path: string) => boolean
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
            <SelectTrigger aria-label="project" className="h-7 min-w-0 flex-1">
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
            className="flex size-7 shrink-0 items-center justify-center rounded border border-[var(--color-border)] hover:bg-[var(--color-accent)]"
          >
            <Plus className="size-3.5" />
          </button>
          {here && !here.builtin && (
            <button
              onClick={() => onRemove(here.id)}
              title="remove this project"
              aria-label="remove project"
              className="flex size-7 shrink-0 items-center justify-center rounded border border-[var(--color-border)] text-[var(--color-muted-foreground)] hover:bg-[var(--color-accent)]"
            >
              <Trash2 className="size-3.5" />
            </button>
          )}
        </div>
        {here && (
          <p className="px-0.5 pt-1 text-[10px] leading-snug text-[var(--color-muted-foreground)]">{here.note}</p>
        )}
      </div>

      <Group label="package" files={files} active={active} onSelect={onSelect} onAdd={onAddFile} onRemove={onRemoveFile} canRemove={canRemove} />
      {/* The host's own data, in its own group. A `.va` never carries somebody's
          wallet, and putting it under the same heading as the package would be
          the one line of this interface that lied. */}
      {/* The same wallet in every project, because a person has one and every
          application they install looks at that one. */}
      {hostFiles?.length ? <Group label="host" files={hostFiles} active={active} onSelect={onSelect} onAdd={onAddFile} onRemove={onRemoveFile} canRemove={canRemove} /> : null}
      {/* And the publisher's own side, which holds their issuer key and never
          goes near the phone. */}
      {serverFiles?.length ? <Group label="server" files={serverFiles} active={active} onSelect={onSelect} onAdd={onAddFile} onRemove={onRemoveFile} canRemove={canRemove} /> : null}
      <div className="mt-auto border-t border-[var(--color-border)] px-3 py-2 text-[10px] leading-snug text-[var(--color-muted-foreground)]">
        A package is several files sharing one scope. What crosses to another
        package is what that package exports.
      </div>
    </aside>
  )
}

function Group({
  label, files, active, onSelect, onAdd, onRemove, canRemove,
}: {
  label: GroupName
  files: Item[]
  active: string
  onSelect: (path: string) => void
  onAdd: (group: GroupName) => void
  onRemove: (path: string) => void
  canRemove: (path: string) => boolean
}) {
  return (
    <>
      <div className="flex items-center justify-between px-3 py-2 text-[10px] font-semibold uppercase tracking-widest text-[var(--color-muted-foreground)]">
        {label}
        <button
          onClick={() => onAdd(label)}
          title={`a file in ${label}`}
          aria-label={`new ${label} file`}
          className="rounded p-0.5 hover:bg-[var(--color-accent)] hover:text-[var(--color-foreground)]"
        >
          <Plus className="size-3" />
        </button>
      </div>
      <ScrollArea className="shrink-0">
        <nav className="px-1.5 pb-2">
          {files.map((f) => {
            const Icon = f.name.endsWith('.json') ? FileJson : f.name.endsWith('.val') ? FileCode2 : FileText
            return (
              // The row is a button and the × is a button, so the × is a
              // sibling rather than a child: nesting one inside the other is
              // invalid, and the browser's repair puts it outside the row.
              <div
                key={f.path}
                className={cn(
                  'group relative rounded transition-colors',
                  active === f.path ? 'bg-[var(--color-accent)]' : 'hover:bg-[var(--color-accent)]/60',
                )}
              >
                <button
                  onClick={() => onSelect(f.path)}
                  className="flex w-full flex-col gap-0.5 px-2 py-1.5 pr-7 text-left"
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
                {canRemove(f.path) && (
                  <button
                    onClick={() => onRemove(f.path)}
                    title="remove this file"
                    aria-label={`remove ${f.name}`}
                    className={cn(
                      'absolute right-1.5 top-1.5 rounded p-0.5 text-[var(--color-muted-foreground)]',
                      'opacity-0 hover:bg-[var(--color-background)] hover:text-[var(--color-foreground)]',
                      'group-hover:opacity-100 focus-visible:opacity-100',
                    )}
                  >
                    <Trash2 className="size-3" />
                  </button>
                )}
              </div>
            )
          })}
        </nav>
      </ScrollArea>
    </>
  )
}

export type { SourceFile }
