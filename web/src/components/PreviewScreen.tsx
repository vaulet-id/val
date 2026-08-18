import * as React from 'react'
import { text as bundle } from '@/examples'
import type { Resolved } from '@/val/wasm'

// The preview is drawn by Flutter, and the wallet it draws from is the host's.
// This side hands over three things and holds none of them: the screens the
// compiler parsed, the signed text bundle, and the file somebody can edit.

export function PreviewScreen({
  screens, locale, dark, onTap,
}: {
  screens: Resolved[]
  locale: 'th' | 'en'
  dark: boolean
  onTap: (action: string) => void
}) {
  const frame = React.useRef<HTMLIFrameElement>(null)
  const [ready, setReady] = React.useState(false)
  const [missing, setMissing] = React.useState(false)

  React.useEffect(() => {
    const onMessage = (e: MessageEvent) => {
      if (typeof e.data !== 'string') return
      try {
        const msg = JSON.parse(e.data)
        if (msg.ready) setReady(true)
        if (msg.type === 'tap' && msg.action) onTap(msg.action)
      } catch {
        /* not ours */
      }
    }
    window.addEventListener('message', onMessage)
    return () => window.removeEventListener('message', onMessage)
  }, [onTap])

  React.useEffect(() => {
    if (!ready) return
    frame.current?.contentWindow?.postMessage(
      JSON.stringify({ screens, text: bundle, locale, dark }),
      '*',
    )
  }, [ready, screens, locale, dark])

  // Not checked in: forty megabytes of CanvasKit, and a repository keeps its
  // history forever. So the panel has to be honest about not being built.
  React.useEffect(() => {
    const timer = setTimeout(() => setMissing(!ready), 2500)
    return () => clearTimeout(timer)
  }, [ready])

  if (missing && !ready)
    return (
      <div className="flex h-full flex-col items-center justify-center gap-2 p-6 text-center">
        <p className="text-xs leading-relaxed text-[var(--color-muted-foreground)]">
          The preview is drawn by Flutter — the same toolkit the wallet uses — and
          it has not been built yet.
        </p>
        <code className="rounded bg-[var(--color-muted)] px-2 py-1 font-mono text-[11px]">
          ./preview/build.sh
        </code>
      </div>
    )

  return (
    <iframe
      ref={frame}
      title="preview"
      src="/preview/index.html"
      className="h-full w-full border-0"
    />
  )
}
