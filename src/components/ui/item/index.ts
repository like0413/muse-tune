import type { VariantProps } from 'class-variance-authority'
import { cva } from 'class-variance-authority'

export { default as Item } from './Item.vue'
export { default as ItemActions } from './ItemActions.vue'
export { default as ItemContent } from './ItemContent.vue'
export { default as ItemDescription } from './ItemDescription.vue'
export { default as ItemFooter } from './ItemFooter.vue'
export { default as ItemGroup } from './ItemGroup.vue'
export { default as ItemHeader } from './ItemHeader.vue'
export { default as ItemMedia } from './ItemMedia.vue'
export { default as ItemSeparator } from './ItemSeparator.vue'
export { default as ItemTitle } from './ItemTitle.vue'

export const itemVariants = cva(
  'group/item flex items-center border border-transparent text-sm rounded-md transition-colors [a]:hover:bg-accent/50 [a]:transition-colors duration-100 flex-wrap outline-none focus-visible:border-ring focus-visible:ring-ring/50 focus-visible:ring-3',
  {
    variants: {
      variant: {
        default: 'bg-transparent',
        outline: 'border-border dark:bg-muted/50',
        muted: 'bg-muted/50',
      },
      size: {
        default: 'p-4 gap-4 ',
        sm: 'py-3 px-4 gap-2.5',
      },
    },
    defaultVariants: {
      variant: 'outline',
      size: 'sm',
    },
  },
)

export const itemMediaVariants = cva(
  'flex shrink-0 items-center justify-center gap-2 [&_svg]:pointer-events-none',
  {
    variants: {
      variant: {
        default:
          'bg-transparent group-has-[[data-slot=item-description]]/item:self-start group-has-[[data-slot=item-description]]/item:translate-y-0.5',
        icon: "size-8 self-center border rounded-sm bg-muted [&_svg:not([class*='size-'])]:size-4",
        image:
          'size-10 rounded-sm overflow-hidden group-has-[[data-slot=item-description]]/item:self-start group-has-[[data-slot=item-description]]/item:translate-y-0.5 [&_img]:size-full [&_img]:object-cover',
      },
    },
    defaultVariants: {
      variant: 'icon',
    },
  },
)

export type ItemVariants = VariantProps<typeof itemVariants>
export type ItemMediaVariants = VariantProps<typeof itemMediaVariants>
