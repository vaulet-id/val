import * as React from 'react'
import Editor, { useMonaco } from '@monaco-editor/react'
import { Navbar, type Mode } from '@/components/Navbar'
import { FileTree } from '@/components/FileTree'
import { PreviewScreen } from '@/components/PreviewScreen'
import { ReportPanel } from '@/components/ReportPanel'
import { DebugPane } from '@/components/DebugPane'
import type { Entry } from '@/components/LogPanel'
import { DocsView } from '@/components/DocsView'
import { Tabs, TabsContent, TabsList, TabsTrigger } from '@/components/ui/tabs'
import { ResizableHandle, ResizablePanel, ResizablePanelGroup } from '@/components/ui/resizable'
import { ScrollArea } from '@/components/ui/scroll-area'
import { files, hostFiles, serverFiles, text as bundle } from '@/examples'
import { registerVal } from '@/val/monaco-lang'
import * as val from '@/val/wasm'
import { runHandler, type Decision } from '@/val/server-runtime'
import { Button } from '@/components/ui/button'
import { Play, Loader2 } from 'lucide-react'

const LOCALES = ['en', 'th']

export default function App() {
  const [mode, setMode] = React.useState<Mode>('playground')
  const [dark, setDark] = React.useState(false)
  const [locale, setLocale] = React.useState<'th' | 'en'>('en')
  const [active, setActive] = React.useState(files[0].path)
  const [sources, setSources] = React.useState<Record<string, string>>(
    Object.fromEntries([...files, ...hostFiles, ...serverFiles].map((f) => [f.path, f.source])),
  )
  const [ready, setReady] = React.useState(false)
  const [tab, setTab] = React.useState('screen')
  const [log, setLog] = React.useState<Entry[]>([])
  const [debug, setDebug] = React.useState(false)
  const [running, setRunning] = React.useState(false)

  const monaco = useMonaco()

  React.useEffect(() => {
    document.documentElement.classList.toggle('dark', dark)
  }, [dark])

  React.useEffect(() => {
    if (monaco) registerVal(monaco)
  }, [monaco])

  // The real compiler, in the page. What a reader is told here is what a host
  // would say, because it is the same code.
  React.useEffect(() => {
    val.load().then(() => setReady(true))
  }, [])

  const all = [...files, ...hostFiles, ...serverFiles]
  const current = all.find((f) => f.path === active) ?? files[0]

  // Editing the host's wallet must not empty the preview. It belongs to no
  // package, so the package under inspection is the last `.val` one chosen —
  // which is also what somebody means by it: they are changing the data behind
  // the screen they were just looking at.
  const [pkg, setPkg] = React.useState(files[0].pkg)
  React.useEffect(() => {
    if (current.path.endsWith('.val')) setPkg(current.pkg)
  }, [current])
  const source = sources[active] ?? ''
  const isVal = active.endsWith('.val')

  // A package is several files sharing one scope, so they are analysed
  // together. `wallet.val` presses an action `loyalty.val` declares; either
  // alone is half a program and fails for the right reason.
  const packageSource = React.useMemo(
    () =>
      files
        .filter((f) => f.pkg === pkg && f.path.endsWith('.val'))
        .map((f) => sources[f.path] ?? '')
        .join('\n'),
    [sources, pkg],
  )

  const analysis = React.useMemo(() => {
    if (!ready) return null
    try {
      return val.analyse(packageSource, bundle, LOCALES)
    } catch {
      return null
    }
  }, [ready, packageSource])

  const wallet = React.useMemo(() => {
    try {
      return JSON.parse(sources['fixtures/wallet.json'] ?? '{}')
    } catch {
      return {}
    }
  }, [sources])

  // Resolved by the compiler, against the wallet somebody edited. The renderer
  // is handed values and draws them; it does not look anything up.
  const resolved = React.useMemo(() => {
    if (!ready || !analysis) return []
    try {
      return val.resolve(packageSource, wallet).screens
    } catch {
      return []
    }
  }, [ready, analysis, packageSource, wallet])

  // One press, both sides. The action runs on the device and the record it
  // produces goes straight to the publisher's handler — which is the whole
  // transaction, and the thing no tool shows in one place today.
  const dispatch = React.useCallback(
    async (action: string) => {
      if (!ready) return
      setRunning(true)
      setDebug(true)

      const run = val.run(packageSource, action, wallet)
      let decision: Decision | undefined
      if (monaco && run.token && run.deviceKey) {
        decision = await runHandler(
          monaco,
          sources['server/handler.ts'] ?? '',
          run.token,
          packageSource,
          run.deviceKey,
        )
      }
      setLog((l) => [...l, { at: Date.now(), run, decision }])
      setRunning(false)
    },
    [ready, packageSource, wallet, monaco, sources],
  )

  /// Build the package, then show it running.
  ///
  /// **Running an app is not running an action.** The checks are what a host
  /// does before admitting a package, and the report is what a person is shown;
  /// after that the application is up, on screen, waiting. An action happens
  /// because somebody presses something — declaring one binds nothing.
  const build = React.useCallback(() => {
    if (!analysis) return
    setDebug(true)
    // On screen when it built, on the problems when it did not.
    setTab(analysis.diagnostics.some((d) => d.severity === 'error') ? 'report' : 'screen')
    setLog((l) => [
      ...l,
      {
        at: Date.now(),
        build: {
          app: analysis.report.app,
          version: analysis.report.version,
          problems: analysis.diagnostics.filter((d) => d.severity === 'error').length,
          warnings: analysis.diagnostics.filter((d) => d.severity === 'warning').length,
          report: analysis.report,
        },
      },
    ])
  }, [analysis])

  // Diagnostics land as markers, so a float is underlined where it was written
  // rather than described in a panel somewhere else. Only for the file being
  // edited: a package's lines are numbered from the joined source.
  React.useEffect(() => {
    if (!monaco || !analysis || !isVal) return
    const model = monaco.editor.getModels().find((m) => m.uri.path.endsWith(current.name))
    if (!model) return
    const inPkg = files.filter((f) => f.pkg === pkg && f.path.endsWith('.val'))
    const before = inPkg
      .slice(0, inPkg.findIndex((f) => f.path === active))
      .reduce((n, f) => n + (sources[f.path] ?? '').split('\n').length + 1, 0)
    const lines = source.split('\n').length

    monaco.editor.setModelMarkers(
      model,
      'val',
      analysis.diagnostics
        .filter((d) => d.line > before && d.line <= before + lines)
        .map((d) => ({
          startLineNumber: d.line - before,
          startColumn: d.column,
          endLineNumber: d.line - before,
          endColumn: d.column + 8,
          message: d.message,
          severity: d.severity === 'error' ? monaco.MarkerSeverity.Error : monaco.MarkerSeverity.Warning,
        })),
    )
  }, [monaco, analysis, active, source, isVal, pkg, sources])

  return (
    <div className="flex h-full flex-col">
      <Navbar
        mode={mode}
        onMode={setMode}
        dark={dark}
        onDark={setDark}
        locale={locale}
        onLocale={setLocale}
        debugger={debug}
        onDebugger={setDebug}
        problems={analysis?.diagnostics.length ?? 0}
      />

      {mode === 'docs' ? (
        <DocsView dark={dark} />
      ) : (
        <ResizablePanelGroup direction="horizontal" autoSaveId="val-playground" className="min-h-0 flex-1">
          <ResizablePanel defaultSize={15} minSize={9} maxSize={30}>
            <FileTree
              heading="package"
              files={files}
              hostFiles={hostFiles}
              serverFiles={serverFiles}
              active={active}
              onSelect={setActive}
            />
          </ResizablePanel>

          <ResizableHandle withHandle />

          <ResizablePanel defaultSize={53} minSize={25}>
            <ResizablePanelGroup direction="vertical" autoSaveId="val-editor">
              <ResizablePanel defaultSize={debug ? 62 : 100} minSize={20}>
            <Editor
              height="100%"
              path={active}
              language={isVal ? 'val' : active.endsWith('.ts') ? 'typescript' : 'json'}
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
                automaticLayout: true,
              }}
            />
              </ResizablePanel>

              {debug && (
                <>
                  <ResizableHandle />
                  <ResizablePanel defaultSize={38} minSize={12}>
                    <DebugPane
                      diagnostics={analysis?.diagnostics ?? []}
                      entries={log}
                      onClear={() => setLog([])}
                      onClose={() => setDebug(false)}
                      onGo={(line) => {
                        const editor = monaco?.editor.getEditors()[0]
                        editor?.revealLineInCenter(line)
                        editor?.setPosition({ lineNumber: line, column: 1 })
                        editor?.focus()
                      }}
                    />
                  </ResizablePanel>
                </>
              )}
            </ResizablePanelGroup>
          </ResizablePanel>

          <ResizableHandle withHandle />

          <ResizablePanel defaultSize={32} minSize={18}>
            <Tabs value={tab} onValueChange={setTab} className="flex h-full min-h-0 flex-col">
              {/* Room above and below the group. Flush against the rule it
                  sits on, a control reads as part of the border rather than as
                  something to press. */}
              <div className="flex shrink-0 items-center gap-2 border-b border-[var(--color-border)] px-2 py-2">
                <TabsList>
                  <TabsTrigger value="screen">Preview</TabsTrigger>
                  <TabsTrigger value="report">Report</TabsTrigger>
                </TabsList>
                {!ready ? (
                  <span className="ml-auto text-[10px] text-[var(--color-muted-foreground)]">loading the compiler…</span>
                ) : (
                  <Button
                    size="sm"
                    className="ml-auto"
                    onClick={build}
                    disabled={!analysis || running}
                    title="checks the package, then shows it running — pressing something is what runs an action"
                  >
                    {running ? <Loader2 className="size-3 animate-spin" /> : <Play className="size-3" />}
                    Build &amp; Run
                  </Button>
                )}
              </div>

              <TabsContent value="screen" className="min-h-0 flex-1">
                <PreviewScreen screens={resolved} locale={locale} dark={dark} onTap={dispatch} />
              </TabsContent>

              <TabsContent value="report" className="min-h-0 flex-1">
                <ScrollArea className="h-full">
                  {analysis && <ReportPanel analysis={analysis} />}
                </ScrollArea>
              </TabsContent>

            </Tabs>
          </ResizablePanel>
        </ResizablePanelGroup>
      )}
    </div>
  )
}
