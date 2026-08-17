import { GripVertical } from 'lucide-react'
import * as ResizablePrimitive from 'react-resizable-panels'
import { cn } from '@/lib/utils'

export const ResizablePanelGroup = ({
  className,
  ...props
}: React.ComponentProps<typeof ResizablePrimitive.PanelGroup>) => (
  <ResizablePrimitive.PanelGroup
    className={cn('flex h-full w-full data-[panel-group-direction=vertical]:flex-col', className)}
    {...props}
  />
)

export const ResizablePanel = ResizablePrimitive.Panel

export const ResizableHandle = ({
  withHandle,
  className,
  ...props
}: React.ComponentProps<typeof ResizablePrimitive.PanelResizeHandle> & { withHandle?: boolean }) => (
  <ResizablePrimitive.PanelResizeHandle
    className={cn(
      'relative flex w-px items-center justify-center bg-[var(--color-border)] transition-colors',
      'after:absolute after:inset-y-0 after:left-1/2 after:w-3 after:-translate-x-1/2',
      'hover:bg-[var(--color-ring)] data-[resize-handle-state=drag]:bg-[var(--color-ring)]',
      'data-[panel-group-direction=vertical]:h-px data-[panel-group-direction=vertical]:w-full',
      className,
    )}
    {...props}
  >
    {withHandle && (
      <div className="z-10 flex h-6 w-2.5 items-center justify-center rounded-sm border border-[var(--color-border)] bg-[var(--color-background)]">
        <GripVertical className="size-2.5 text-[var(--color-muted-foreground)]" />
      </div>
    )}
  </ResizablePrimitive.PanelResizeHandle>
)
