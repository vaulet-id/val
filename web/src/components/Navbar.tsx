import { Tabs, TabsList, TabsTrigger } from '@/components/ui/tabs'
import { Button } from '@/components/ui/button'
import { Moon, Sun, Languages, PanelBottom } from 'lucide-react'

export type Mode = 'playground' | 'docs'

export function Navbar({
  mode, onMode, dark, onDark, locale, onLocale, debugger: showDebugger, onDebugger, problems,
}: {
  mode: Mode
  onMode: (m: Mode) => void
  dark: boolean
  onDark: (d: boolean) => void
  locale: 'th' | 'en'
  onLocale: (l: 'th' | 'en') => void
  debugger: boolean
  onDebugger: (open: boolean) => void
  problems: number
}) {
  return (
    <header className="flex h-11 shrink-0 items-center gap-4 border-b border-[var(--color-border)] px-3">
      <div className="flex items-baseline gap-2">
        <span className="font-mono text-sm font-bold tracking-tight">VAL</span>
        <span className="hidden text-xs text-[var(--color-muted-foreground)] sm:inline">
          a language for applications whose execution can be proved
        </span>
      </div>

      <Tabs value={mode} onValueChange={(v) => onMode(v as Mode)} className="ml-auto">
        <TabsList>
          <TabsTrigger value="playground">Playground</TabsTrigger>
          <TabsTrigger value="docs">Docs</TabsTrigger>
        </TabsList>
      </Tabs>

      {/* The pane under the editor: what the compiler said, and what a press
          did. Off by default, because the first thing to read is the program. */}
      <Button
        variant={showDebugger ? 'outline' : 'ghost'}
        size="sm"
        onClick={() => onDebugger(!showDebugger)}
      >
        <PanelBottom className="size-3.5" />
        Debugger
        {problems > 0 && (
          <span className="rounded bg-red-500/15 px-1 text-[10px] font-medium text-red-500">{problems}</span>
        )}
      </Button>

      {/* The preview renders in the viewer's locale, because the host formats
          text and the application never touches it. Switching here is how you
          see that a missing translation is a failed build. */}
      <Button variant="ghost" size="sm" onClick={() => onLocale(locale === 'th' ? 'en' : 'th')}>
        <Languages className="size-3.5" />
        {locale.toUpperCase()}
      </Button>
      <Button variant="ghost" size="icon" onClick={() => onDark(!dark)} aria-label="theme">
        {dark ? <Sun className="size-3.5" /> : <Moon className="size-3.5" />}
      </Button>
    </header>
  )
}
