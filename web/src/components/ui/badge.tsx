import { cva, type VariantProps } from 'class-variance-authority'
import { cn } from '@/lib/utils'

const badgeVariants = cva(
  'inline-flex items-center rounded border px-1.5 py-0.5 text-[10px] font-medium leading-none',
  {
    variants: {
      variant: {
        default: 'border-[var(--color-border)] text-[var(--color-muted-foreground)]',
        // The three grades of data. They are the reason this panel exists, so
        // they are the only place colour carries meaning.
        verified: 'border-[var(--color-verified)] text-[var(--color-verified)]',
        self: 'border-[var(--color-selfasserted)] text-[var(--color-selfasserted)]',
        origin: 'border-[var(--color-origin)] text-[var(--color-origin)]',
      },
    },
    defaultVariants: { variant: 'default' },
  },
)

export function Badge({
  className,
  variant,
  ...props
}: React.HTMLAttributes<HTMLSpanElement> & VariantProps<typeof badgeVariants>) {
  return <span className={cn(badgeVariants({ variant }), className)} {...props} />
}
