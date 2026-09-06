<script setup lang="ts">
import { Languages } from '@lucide/vue'

import {
  Item,
  ItemActions,
  ItemContent,
  ItemDescription,
  ItemMedia,
  ItemTitle,
} from '@/components/ui/item'
import { Tabs, TabsList, TabsTrigger } from '@/components/ui/tabs'
</script>

<template>
  <Item>
    <ItemMedia class="icon-tone-sky-500">
      <Languages />
    </ItemMedia>
    <ItemContent>
      <ItemTitle>界面语言</ItemTitle>
      <ItemDescription>切换语言后会立即生效</ItemDescription>
    </ItemContent>
    <ItemActions>
      <Tabs default-value="zh">
        <TabsList>
          <TabsTrigger value="zh">简体中文</TabsTrigger>
          <TabsTrigger value="en">English</TabsTrigger>
        </TabsList>
      </Tabs>
    </ItemActions>
  </Item>
</template>
