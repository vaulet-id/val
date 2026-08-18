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
import { ALLOWED, blankFile, bundleOf, examples, type Group, HOST, hostFiles, newProject, type Project } from '@/examples'
import { registerVal } from '@/val/monaco-lang'
import * as val from '@/val/wasm'
import { runHandler, type Decision } from '@/val/server-runtime'
import { Button } from '@/components/ui/button'
import { Play, Loader2 } from 'lucide-react'

const LOCALES = ['en', 'th']

/// Which grammar Monaco highlights a file with. A server may be written in any
/// of the four languages the runner accepts, and a Go file highlighted as
/// TypeScript is underlined red from its first line.
const GRAMMAR: Record<string, string> = {
  val: 'val',
  ts: 'typescript',
  go: 'go',
  rs: 'rust',
  py: 'python',
  json: 'json',
}

const languageOf = (path: string) => GRAMMAR[path.split('.').pop() ?? ''] ?? 'plaintext'

export default function App() {
  const [mode, setMode] = React.useState<Mode>('playground')
  const [dark, setDark] = React.useState(false)
  const [locale, setLocale] = React.useState<'th' | 'en'>('en')
  const [active, setActive] = React.useState(examples[0].files[0].path)
  const [sources, setSources] = React.useState<Record<string, string>>(
    Object.fromEntries(
      [...examples.flatMap((p) => [...p.files, ...p.servers]), ...hostFiles].map((f) => [f.path, f.source]),
    ),
  )
  const [ready, setReady] = React.useState(false)
  const [tab, setTab] = React.useState('screen')
  const [log, setLog] = React.useState<Entry[]>([])
  const [debug, setDebug] = React.useState(false)
  /// One project at a time, which is how anybody actually works. A project is a
  /// package, the wallet it looks at, and the publisher's own server.
  const [projects, setProjects] = React.useState<Project[]>(examples)
  /// The host's files are not part of any project — a person has one phone —
  /// so they live beside the projects rather than inside one.
  const [hosts, setHosts] = React.useState(hostFiles)
  const [project, setProject] = React.useState(examples[0].id)
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

  const here = projects.find((p) => p.id === project) ?? projects[0]
  const packageFiles = here.files
  const serverFiles = here.servers
  const all = [...packageFiles, ...hosts, ...serverFiles]
  const current = all.find((f) => f.path === active) ?? packageFiles[0]

  // Opening a project opens its first file, or the editor shows one that is no
  // longer in the tree.
  React.useEffect(() => {
    if (!all.some((f) => f.path === active)) setActive(packageFiles[0]?.path ?? active)
  }, [project])

  // Editing the host's wallet must not empty the preview. It belongs to no
  // package, so the package under inspection is the last `.val` one chosen —
  // which is also what somebody means by it: they are changing the data behind
  // the screen they were just looking at.
  const source = sources[active] ?? ''
  const isVal = active.endsWith('.val')

  // A package is several files sharing one scope, so they are analysed
  // together. `wallet.val` presses an action `loyalty.val` declares; either
  // alone is half a program and fails for the right reason.
  const packageSource = React.useMemo(
    () => packageFiles.filter((f) => f.path.endsWith('.val')).map((f) => sources[f.path] ?? '').join('\n'),
    [sources, packageFiles],
  )

  const bundle = React.useMemo(() => bundleOf(packageFiles, sources), [packageFiles, sources])

  const analysis = React.useMemo(() => {
    if (!ready) return null
    try {
      return val.analyse(packageSource, bundle.keys, bundle.locales)
    } catch {
      return null
    }
  }, [ready, packageSource, bundle])

  const wallet = React.useMemo(() => {
    try {
      return JSON.parse(sources[HOST] ?? '{}')
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
      if (run.token && run.deviceKey) {
        decision = await runHandler(
          here.servers.map((f) => ({ name: f.name, source: sources[f.path] ?? '' })),
          run.token,
          packageSource,
          run.deviceKey,
        )
      }
      // A committed action changes the wallet, so the wallet file changes. The
      // preview reads state from there and nowhere else — keeping a second copy
      // in React is how a screen comes to disagree with the record that was
      // just signed.
      if (run.outcome?.kind === 'committed' && run.after) {
        setSources((all) => {
          const held = JSON.parse(all[HOST] ?? '{}')
          // Merged, not replaced: the file is one phone and every project on it
          // writes here. A run reads back only the fields its own program
          // declares, so the projects do not see each other — unless two of
          // them name a field the same, which in a real wallet they could not,
          // because state is kept per install.
          const state = { ...(held.state ?? {}), ...(run.after as object) }
          return { ...all, [HOST]: JSON.stringify({ ...held, state }, null, 2) }
        })
      }

      setLog((l) => [...l, { at: Date.now(), run, decision }])
      setRunning(false)
    },
    [ready, packageSource, wallet, monaco, sources, here],
  )

  /// A project of somebody's own. The examples are a starting point; this is
  /// the point.
  const addProject = React.useCallback(() => {
    const n = projects.filter((p) => !p.builtin).length + 1
    const id = `app-${n}`
    const made = newProject(id, `New app ${n}`)
    setProjects((all) => [...all, made])
    setSources((s) => ({
      ...s,
      ...Object.fromEntries([...made.files, ...made.servers].map((f) => [f.path, f.source])),
    }))
    setProject(id)
    setActive(made.files[0].path)
  }, [projects])

  /// A file in one of the three groups.
  ///
  /// Named by the person adding it, because the name is the only thing about a
  /// file the playground cannot guess — a package is analysed as one scope, and
  /// a server file is imported by the name it is saved under.
  const addFile = React.useCallback(
    (group: Group) => {
      const name = window.prompt(`New file in ${group} — ${ALLOWED[group].join(' or ')}`)?.trim()
      if (!name) return
      if (!ALLOWED[group].some((ext) => name.endsWith(ext))) {
        window.alert(`${group} files end in ${ALLOWED[group].join(' or ')}`)
        return
      }

      const made = blankFile(group, here.id, name)
      const taken = [...packageFiles, ...hosts, ...serverFiles].some((f) => f.path === made.path)
      if (taken) {
        window.alert(`${name} is already here`)
        return
      }

      // A server has one entry point, so adding a handler in another language
      // is how you change language: the old one goes. Two would leave the
      // runner choosing for you, which is not a choice it can make correctly.
      const replaced = group === 'server' && name.startsWith('handler.')
        ? serverFiles.find((f) => f.name.startsWith('handler.'))
        : undefined
      if (replaced && !window.confirm(`Replace ${replaced.name} with ${name}?`)) return

      setSources((all) => {
        const next = { ...all, [made.path]: made.source }
        if (replaced) delete next[replaced.path]
        return next
      })
      if (group === 'host') setHosts((all) => [...all, made])
      else {
        setProjects((all) =>
          all.map((p) =>
            p.id !== here.id
              ? p
              : group === 'package'
                ? { ...p, files: [...p.files, made] }
                : { ...p, servers: [...p.servers.filter((f) => f.path !== replaced?.path), made] },
          ),
        )
      }
      setActive(made.path)
    },
    [here, packageFiles, hosts, serverFiles],
  )

  /// What can go. An example is what the documentation points at, so its own
  /// files stay; a file added here is the person's own wherever they added it.
  const removable = React.useCallback(
    (path: string) => {
      const file = [...packageFiles, ...hosts, ...serverFiles].find((f) => f.path === path)
      if (!file) return false
      if (file.added) return true
      return !here.builtin && file.pkg !== 'host'
    },
    [here, packageFiles, hosts, serverFiles],
  )

  /// Removing a file, except the three the playground cannot do without: the
  /// wallet the preview reads, the last `.val` of a package, and the server's
  /// entry point.
  const removeFile = React.useCallback(
    (path: string) => {
      if (!removable(path)) return
      if (path === HOST) return window.alert('the preview reads the wallet from this file')
      const inPackage = packageFiles.some((f) => f.path === path)
      if (inPackage && path.endsWith('.val') && packageFiles.filter((f) => f.path.endsWith('.val')).length === 1) {
        return window.alert('a package is at least one .val')
      }
      if (path.endsWith('handler.ts')) return window.alert('handler.ts is what a record is sent to')

      setSources((all) => {
        const rest = { ...all }
        delete rest[path]
        return rest
      })
      setHosts((all) => all.filter((f) => f.path !== path))
      setProjects((all) =>
        all.map((p) =>
          p.id !== here.id
            ? p
            : { ...p, files: p.files.filter((f) => f.path !== path), servers: p.servers.filter((f) => f.path !== path) },
        ),
      )
      if (active === path) setActive(packageFiles[0]?.path ?? HOST)
    },
    [here, packageFiles, active, removable],
  )

  const removeProject = React.useCallback(
    (id: string) => {
      setProjects((all) => all.filter((p) => p.id !== id))
      setProject((current) => (current === id ? examples[0].id : current))
    },
    [],
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
    const inPkg = packageFiles.filter((f) => f.path.endsWith('.val'))
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
  }, [monaco, analysis, active, source, isVal, packageFiles, sources])

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
        <DocsView dark={dark} locale={locale} />
      ) : (
        <ResizablePanelGroup direction="horizontal" autoSaveId="val-playground" className="min-h-0 flex-1">
          <ResizablePanel defaultSize={15} minSize={9} maxSize={30}>
            <FileTree
              projects={projects}
              project={project}
              onProject={setProject}
              onNew={addProject}
              onRemove={removeProject}
              files={packageFiles}
              hostFiles={hosts}
              serverFiles={serverFiles}
              active={active}
              onSelect={setActive}
              onAddFile={addFile}
              onRemoveFile={removeFile}
              canRemove={removable}
            />
          </ResizablePanel>

          <ResizableHandle withHandle />

          <ResizablePanel defaultSize={53} minSize={25}>
            <ResizablePanelGroup direction="vertical" autoSaveId="val-editor">
              <ResizablePanel defaultSize={debug ? 62 : 100} minSize={20}>
            <Editor
              height="100%"
              path={active}
              language={languageOf(active)}
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
                <PreviewScreen screens={resolved} text={bundle.keys} locale={locale} dark={dark} onTap={dispatch} />
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
