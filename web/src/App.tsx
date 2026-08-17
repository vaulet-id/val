import * as React from 'react'
import Editor, { useMonaco } from '@monaco-editor/react'
import { Navbar, type Mode } from '@/components/Navbar'
import { FileTree } from '@/components/FileTree'
import { PreviewScreen } from '@/components/PreviewScreen'
import { ReportPanel } from '@/components/ReportPanel'
import { RunPanel } from '@/components/RunPanel'
import { Button } from '@/components/ui/button'
import { Play, Loader2 } from 'lucide-react'
import { DocsView } from '@/components/DocsView'
import { Tabs, TabsContent, TabsList, TabsTrigger } from '@/components/ui/tabs'
import { ResizableHandle, ResizablePanel, ResizablePanelGroup } from '@/components/ui/resizable'
import { ScrollArea } from '@/components/ui/scroll-area'
import { files } from '@/examples'
import { parse } from '@/val/parse'
import { report } from '@/val/report'
import { build, run, type BuildResult, type RunResult } from '@/val/run'
import { registerVal } from '@/val/monaco-lang'

export default function App() {
  const [mode, setMode] = React.useState<Mode>('playground')
  const [dark, setDark] = React.useState(true)
  // English is the base: this is a public repository and a reader arrives from
  // anywhere. Thai is one click away, which is what the switch is for — and
  // switching is how you see that a missing translation fails the build.
  const [locale, setLocale] = React.useState<'th' | 'en'>('en')
  const [active, setActive] = React.useState(files[0].path)
  const [sources, setSources] = React.useState<Record<string, string>>(
    Object.fromEntries(files.map((f) => [f.path, f.source])),
  )
  const monaco = useMonaco()

  React.useEffect(() => {
    document.documentElement.classList.toggle('dark', dark)
  }, [dark])

  React.useEffect(() => {
    if (monaco) registerVal(monaco)
  }, [monaco])

  const source = sources[active] ?? ''
  const isVal = active.endsWith('.val')
  const program = React.useMemo(() => (isVal ? parse(source) : { decls: [], diagnostics: [] }), [source, isVal])
  const rep = React.useMemo(() => report(program), [program])

  const [tab, setTab] = React.useState('screen')
  const [running, setRunning] = React.useState(false)
  const [built, setBuilt] = React.useState<BuildResult | null>(null)
  const [ran, setRan] = React.useState<RunResult | null>(null)

  // Build first, and run only what built — the host runs the same checks and
  // would refuse the package, so running past a failed build would be a lie
  // told by the tooling.
  const buildAndRun = React.useCallback(async () => {
    setRunning(true)
    setTab('run')
    const b = build(program)
    setBuilt(b)
    setRan(b.ok ? await run(program) : null)
    setRunning(false)
  }, [program])

  // Diagnostics from the lexer land as markers, so a float is underlined where
  // it was written rather than described in a panel somewhere else.
  React.useEffect(() => {
    if (!monaco) return
    const model = monaco.editor.getModels()[0]
    if (!model) return
    monaco.editor.setModelMarkers(model, 'val', [
      ...program.diagnostics.map((d) => ({
        startLineNumber: d.line, startColumn: d.column,
        endLineNumber: d.line, endColumn: d.column + 8,
        message: d.message, severity: monaco.MarkerSeverity.Error,
      })),
      ...rep.findings.filter((f) => f.line > 0).map((f) => ({
        startLineNumber: f.line, startColumn: 1,
        endLineNumber: f.line, endColumn: 200,
        message: f.message,
        severity: f.severity === 'error' ? monaco.MarkerSeverity.Error : monaco.MarkerSeverity.Warning,
      })),
    ])
  }, [monaco, program, rep])

  return (
    <div className="flex h-full flex-col">
      <Navbar mode={mode} onMode={setMode} dark={dark} onDark={setDark} locale={locale} onLocale={setLocale} />

      {mode === 'docs' ? (
        <DocsView />
      ) : (
        <ResizablePanelGroup direction="horizontal" autoSaveId="val-playground" className="min-h-0 flex-1">
          {/* Which panel deserves the space depends on what the reader is doing
              — reading the code, watching the screen, or arguing with the
              report — and that is not ours to decide. Sizes persist, because
              re-dragging them every reload is a small tax paid forever. */}
          <ResizablePanel defaultSize={15} minSize={9} maxSize={30}>
            <FileTree heading="package" files={files} active={active} onSelect={setActive} />
          </ResizablePanel>

          <ResizableHandle withHandle />

          <ResizablePanel defaultSize={53} minSize={25}>
            <Editor
              height="100%"
              path={active}
              language={isVal ? 'val' : 'json'}
              theme={dark ? 'val-dark' : 'val-light'}
              value={source}
              onChange={(v) => setSources((s) => ({ ...s, [active]: v ?? '' }))}
              options={{
                fontSize: 12.5,
                fontFamily: 'SF Mono, JetBrains Mono, Menlo, monospace',
                minimap: { enabled: false },
                scrollBeyondLastLine: false,
                renderLineHighlight: 'none',
                padding: { top: 12 },
                lineNumbersMinChars: 3,
                tabSize: 2,
                // Monaco measures its own container, so it has to be told the
                // container is no longer the size it was.
                automaticLayout: true,
              }}
            />
          </ResizablePanel>

          <ResizableHandle withHandle />

          <ResizablePanel defaultSize={32} minSize={18}>
            <Tabs value={tab} onValueChange={setTab} className="flex h-full min-h-0 flex-col">
              <div className="flex h-9 shrink-0 items-center gap-2 border-b border-[var(--color-border)] px-2">
                <TabsList>
                  <TabsTrigger value="screen">Preview</TabsTrigger>
                  <TabsTrigger value="report">Report</TabsTrigger>
                  <TabsTrigger value="run">Run</TabsTrigger>
                </TabsList>
                <Button size="sm" className="ml-auto" onClick={buildAndRun} disabled={running || !isVal}>
                  {running ? <Loader2 className="size-3 animate-spin" /> : <Play className="size-3" />}
                  Build &amp; Run
                </Button>
              </div>
              {/* No ScrollArea: the frame scrolls itself, and two scrollers
                  over one surface is the thing every embedded view gets wrong. */}
              <TabsContent value="screen" className="min-h-0 flex-1">
                <PreviewScreen program={program} locale={locale} dark={dark} />
              </TabsContent>
              <TabsContent value="report" className="min-h-0 flex-1">
                <ScrollArea className="h-full">
                  <ReportPanel report={rep} />
                </ScrollArea>
              </TabsContent>
              <TabsContent value="run" className="min-h-0 flex-1">
                <ScrollArea className="h-full">
                  {built ? (
                    <RunPanel build={built} result={ran} />
                  ) : (
                    <p className="p-4 text-[11px] leading-relaxed text-[var(--color-muted-foreground)]">
                      Build &amp; Run compiles what a host would check, then walks one
                      action: phases in order, effects requested and never performed,
                      and the Merkle root of the state it would commit.
                    </p>
                  )}
                </ScrollArea>
              </TabsContent>
            </Tabs>
          </ResizablePanel>
        </ResizablePanelGroup>
      )}
    </div>
  )
}
