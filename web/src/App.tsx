import * as React from 'react'
import Editor, { useMonaco } from '@monaco-editor/react'
import { Navbar, type Mode } from '@/components/Navbar'
import { FileTree } from '@/components/FileTree'
import { PreviewScreen } from '@/components/PreviewScreen'
import { ReportPanel } from '@/components/ReportPanel'
import { DocsView } from '@/components/DocsView'
import { Tabs, TabsContent, TabsList, TabsTrigger } from '@/components/ui/tabs'
import { ScrollArea } from '@/components/ui/scroll-area'
import { files } from '@/examples'
import { parse } from '@/val/parse'
import { report } from '@/val/report'
import { registerVal } from '@/val/monaco-lang'

export default function App() {
  const [mode, setMode] = React.useState<Mode>('playground')
  const [dark, setDark] = React.useState(true)
  const [locale, setLocale] = React.useState<'th' | 'en'>('th')
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
        <div className="flex min-h-0 flex-1">
          <FileTree heading="package" files={files} active={active} onSelect={setActive} />

          <div className="min-w-0 flex-1 border-r border-[var(--color-border)]">
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
              }}
            />
          </div>

          <div className="flex w-[380px] shrink-0 flex-col">
            <Tabs defaultValue="screen" className="flex min-h-0 flex-1 flex-col">
              <div className="flex h-9 shrink-0 items-center border-b border-[var(--color-border)] px-2">
                <TabsList>
                  <TabsTrigger value="screen">Preview</TabsTrigger>
                  <TabsTrigger value="report">Report</TabsTrigger>
                </TabsList>
              </div>
              <TabsContent value="screen" className="min-h-0 flex-1">
                <ScrollArea className="h-full">
                  <PreviewScreen program={program} locale={locale} />
                </ScrollArea>
              </TabsContent>
              <TabsContent value="report" className="min-h-0 flex-1">
                <ScrollArea className="h-full">
                  <ReportPanel report={rep} />
                </ScrollArea>
              </TabsContent>
            </Tabs>
          </div>
        </div>
      )}
    </div>
  )
}
