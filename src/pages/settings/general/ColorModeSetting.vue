<script setup lang="ts">
import { MoonStar } from '@lucide/vue'

import {
  Item,
  ItemActions,
  ItemContent,
  ItemDescription,
  ItemMedia,
  ItemTitle,
} from '@/components/ui/item'
import { Tabs, TabsList, TabsTrigger } from '@/components/ui/tabs'
import { colorMode } from '@/lib/color-mode'

const selectedColorMode = colorMode.store
</script>

<template>
  <Item>
    <ItemMedia class="icon-tone-violet-500">
      <MoonStar />
    </ItemMedia>
    <ItemContent>
      <ItemTitle>颜色模式</ItemTitle>
      <ItemDescription>切换颜色模式后会立即生效</ItemDescription>
    </ItemContent>
    <ItemActions>
      <Tabs v-model="selectedColorMode">
        <TabsList>
          <TabsTrigger value="auto">跟随系统</TabsTrigger>
          <TabsTrigger value="dark">深色</TabsTrigger>
          <TabsTrigger value="light">浅色</TabsTrigger>
        </TabsList>
      </Tabs>
    </ItemActions>
  </Item>
</template>
