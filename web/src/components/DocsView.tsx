import * as React from 'react'
import { marked } from 'marked'
import { ScrollArea } from '@/components/ui/scroll-area'
import { FileTree } from '@/components/FileTree'
import { docs } from '@/examples'

export function DocsView() {
  const [active, setActive] = React.useState(docs[0].path)
  const doc = docs.find((d) => d.path === active) ?? docs[0]
  const html = React.useMemo(() => marked.parse(doc.source) as string, [doc])

  return (
    <div className="flex min-h-0 flex-1">
      <FileTree
        heading="documents"
        files={docs.map((d) => ({ path: d.path, name: d.name, note: d.path }))}
        active={active}
        onSelect={setActive}
      />
      <ScrollArea className="flex-1">
        <article
          className="prose-val mx-auto max-w-3xl px-8 py-8"
          dangerouslySetInnerHTML={{ __html: html }}
        />
      </ScrollArea>
    </div>
  )
}
