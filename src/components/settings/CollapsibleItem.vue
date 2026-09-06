<script setup lang="ts">
import { ChevronDown } from '@lucide/vue'

import { Button } from '@/components/ui/button'
import { Collapsible, CollapsibleContent, CollapsibleTrigger } from '@/components/ui/collapsible'
import { Item } from '@/components/ui/item'

defineSlots<{
  /** Item 顶部触发区域原有的内容。 */
  default(): unknown
  /** 展开后放置在统一内边距容器中的内容。 */
  content(): unknown
}>()
</script>

<template>
  <Collapsible>
    <Item class="gap-0 overflow-hidden p-0">
      <CollapsibleTrigger as-child>
        <Button
          variant="ghost"
          class="group/collapsible-item h-auto w-full justify-start gap-2.5 rounded-none px-4 py-3 text-left whitespace-normal has-[>svg]:px-4"
          type="button"
        >
          <slot />
          <ChevronDown
            class="text-muted-foreground shrink-0 transition-transform duration-200 group-data-[state=open]/collapsible-item:rotate-180"
            data-icon="inline-end"
            aria-hidden="true"
          />
        </Button>
      </CollapsibleTrigger>

      <CollapsibleContent
        class="data-[state=closed]:animate-collapsible-up data-[state=open]:animate-collapsible-down w-full overflow-hidden"
      >
        <div class="p-3">
          <slot name="content" />
        </div>
      </CollapsibleContent>
    </Item>
  </Collapsible>
</template>
