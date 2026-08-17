import { Tabs, TabsList, TabsTrigger } from '@/components/ui/tabs'
import { Button } from '@/components/ui/button'
import { Moon, Sun, Languages } from 'lucide-react'

export type Mode = 'playground' | 'docs'

export function Navbar({
  mode, onMode, dark, onDark, locale, onLocale,
}: {
  mode: Mode
  onMode: (m: Mode) => void
  dark: boolean
  onDark: (d: boolean) => void
  locale: 'th' | 'en'
  onLocale: (l: 'th' | 'en') => void
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
