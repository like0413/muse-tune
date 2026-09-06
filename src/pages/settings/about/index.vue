<script setup lang="ts">
import { Badge } from '@/components/ui/badge'
import {
  Card,
  CardAction,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from '@/components/ui/card'
import { Separator } from '@/components/ui/separator'
</script>

<template>
  <Card>
    <CardHeader>
      <CardTitle>Muse Tune</CardTitle>
      <CardDescription>为 Windows 11 任务栏打造的轻量媒体控制器。</CardDescription>
      <CardAction>
        <Badge>v0.1.0</Badge>
      </CardAction>
    </CardHeader>
    <CardContent class="grid gap-4 text-sm">
      <Separator />
      <dl class="grid gap-3">
        <div class="flex items-center justify-between gap-4">
          <dt class="text-muted-foreground">平台</dt>
          <dd>Windows 11</dd>
        </div>
        <div class="flex items-center justify-between gap-4">
          <dt class="text-muted-foreground">技术栈</dt>
          <dd>Tauri · Vue</dd>
        </div>
        <div class="flex items-center justify-between gap-4">
          <dt class="text-muted-foreground">许可证</dt>
          <dd>MIT</dd>
        </div>
      </dl>
    </CardContent>
  </Card>
</template>
