import * as React from 'react'
import { text as bundle } from '@/examples'
import type { Decl, Node, Program } from '@/val/parse'

// The preview is drawn by Flutter, not by this file.
//
// An HTML facsimile agreed with itself and was wrong about everything that
// matters: wrapping, Thai line breaking, touch target sizes, what a Material
// button actually looks like. The host's toolkit is the host's toolkit — so the
// panel is the real catalogue behind an iframe, and this side's whole job is to
// hand it the screen the compiler parsed.

type Serialised = {
  name: string
  data: { name: string; source: string; type?: string; policy?: string; audience?: string }[]
  tree: unknown[]
}

function args(node: Node): Record<string, string> {
  const out: Record<string, string> = {}
  node.args.forEach((a, i) => {
    out[a.name ?? String(i)] = a.value.replace(/"/g, '')
  })
  // `tab("history")` names its label positionally; Flutter reads `text`.
  if (out['0'] && !out.text) out.text = out['0']
  return out
}

const serialise = (node: Node): unknown => ({
  kind: node.kind,
  args: args(node),
  children: node.children.map(serialise),
})

export function PreviewScreen({ program, locale, dark }: { program: Program; locale: 'th' | 'en'; dark: boolean }) {
  const frame = React.useRef<HTMLIFrameElement>(null)
  const [ready, setReady] = React.useState(false)
  const [tapped, setTapped] = React.useState<string | null>(null)

  const screens: Serialised[] = React.useMemo(
    () =>
      (program.decls.filter((d) => d.t === 'screen') as Extract<Decl, { t: 'screen' }>[]).map((s) => ({
        name: s.name,
        data: s.data.map((d) => ({
          name: d.name,
          source: d.source,
          type: d.credentialType,
          policy: d.policy,
          audience: d.audience,
        })),
        tree: s.tree.map(serialise),
      })),
    [program],
  )

  React.useEffect(() => {
    const onMessage = (e: MessageEvent) => {
      if (typeof e.data !== 'string') return
      try {
        const msg = JSON.parse(e.data)
        if (msg.ready) setReady(true)
        if (msg.type === 'tap') setTapped(msg.action ?? 'an action')
      } catch {
        /* not ours */
      }
    }
    window.addEventListener('message', onMessage)
    return () => window.removeEventListener('message', onMessage)
  }, [])

  React.useEffect(() => {
    if (!ready) return
    frame.current?.contentWindow?.postMessage(
      JSON.stringify({ screens, text: bundle, locale, dark }),
      '*',
    )
  }, [ready, screens, locale, dark])

  // The preview is built, not checked in: it is forty megabytes of CanvasKit,
  // and a repository carries its history forever. So the panel has to be honest
  // about the case where nobody has built it.
  const [missing, setMissing] = React.useState(false)
  React.useEffect(() => {
    const timer = setTimeout(() => setMissing((m) => (ready ? false : !m ? true : m)), 2500)
    return () => clearTimeout(timer)
  }, [ready])

  if (missing && !ready) {
    return (
      <div className="flex h-full flex-col items-center justify-center gap-2 p-6 text-center">
        <p className="text-xs leading-relaxed text-[var(--color-muted-foreground)]">
          The preview is drawn by Flutter — the same toolkit the wallet uses — and it
          has not been built yet. It is not checked in, because it is forty megabytes
          of CanvasKit and a repository keeps its history forever.
        </p>
        <code className="rounded bg-[var(--color-muted)] px-2 py-1 font-mono text-[11px]">
          ./preview/build.sh
        </code>
      </div>
    )
  }

  return (
    <div className="flex h-full flex-col">
      <iframe
        ref={frame}
        title="preview"
        src="/preview/index.html"
        className="min-h-0 w-full flex-1 border-0"
      />
      <div className="shrink-0 border-t border-[var(--color-border)] px-3 py-1.5 text-[10px] leading-snug text-[var(--color-muted-foreground)]">
        {tapped ? (
          <>
            <span className="font-mono">{tapped}</span> would run here — through require → verify →
            compute → update → execute, with the same consent and the same record.
          </>
        ) : (
          <>Drawn by Flutter, the same toolkit the wallet uses. Nothing executes.</>
        )}
      </div>
    </div>
  )
}
