<script setup lang="ts">
import { Languages, MoonStar, Rocket } from '@lucide/vue'
import { disable, enable, isEnabled } from '@tauri-apps/plugin-autostart'
import { onMounted, shallowRef } from 'vue'

import {
  Item,
  ItemActions,
  ItemContent,
  ItemDescription,
  ItemMedia,
  ItemTitle,
} from '@/components/ui/item'
import { Switch } from '@/components/ui/switch'
import { Tabs, TabsList, TabsTrigger } from '@/components/ui/tabs'
import { colorMode } from '@/lib/color-mode'

const selectedColorMode = colorMode.store

const autostartEnabled = shallowRef(false)

/** 读取 Windows 中真实的开机自启状态。 */
async function loadAutostartState() {
  autostartEnabled.value = await isEnabled()
}

/** 更新 Windows 开机自启注册，并仅在成功后提交界面状态。 */
async function setAutostart(enabledValue: boolean) {
  if (enabledValue === autostartEnabled.value) return

  if (enabledValue) {
    await enable()
  } else {
    await disable()
  }

  autostartEnabled.value = enabledValue
}

onMounted(loadAutostartState)
</script>

<template>
  <div class="flex w-full flex-col gap-3">
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
    <Item>
      <ItemMedia class="icon-tone-emerald-500">
        <Rocket />
      </ItemMedia>
      <ItemContent>
        <ItemTitle>开机自启</ItemTitle>
        <ItemDescription>开机后自动启动 Muse Tune</ItemDescription>
      </ItemContent>
      <ItemActions>
        <Switch
          aria-label="开机自启"
          :model-value="autostartEnabled"
          @update:model-value="setAutostart"
        />
      </ItemActions>
    </Item>
  </div>
</template>
