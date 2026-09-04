<script setup lang="ts">
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from '@/components/ui/card'
</script>

<template>
  <main class="bg-background text-foreground min-h-full p-6">
    <Card class="mx-auto w-full max-w-2xl">
      <CardHeader>
        <CardTitle>设置</CardTitle>
        <CardDescription>管理 Muse Tune 的任务栏播放器。</CardDescription>
      </CardHeader>
      <CardContent>
        <p class="text-muted-foreground text-sm">暂无可配置项。</p>
      </CardContent>
    </Card>
  </main>
</template>
