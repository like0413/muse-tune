<script setup lang="ts">
import { PanelLeft, PanelRight } from '@lucide/vue'

import { Badge } from '@/components/ui/badge'
import {
  Card,
  CardAction,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from '@/components/ui/card'
import {
  Field,
  FieldContent,
  FieldDescription,
  FieldGroup,
  FieldTitle,
} from '@/components/ui/field'
import { Switch } from '@/components/ui/switch'
import { ToggleGroup, ToggleGroupItem } from '@/components/ui/toggle-group'
</script>

<template>
  <div class="grid gap-4">
    <Card>
      <CardHeader>
        <CardTitle>位置与布局</CardTitle>
        <CardDescription>选择播放器停靠在任务栏的哪一侧。</CardDescription>
        <CardAction>
          <Badge variant="outline">当前：左侧</Badge>
        </CardAction>
      </CardHeader>
      <CardContent>
        <FieldGroup>
          <Field data-disabled="true">
            <FieldContent>
              <FieldTitle>任务栏位置</FieldTitle>
              <FieldDescription>当前位置暂时固定为左侧，后续会接入持久化配置。</FieldDescription>
            </FieldContent>
            <ToggleGroup
              type="single"
              default-value="left"
              variant="outline"
              class="w-full"
              disabled
            >
              <ToggleGroupItem value="left" class="flex-1">
                <PanelLeft class="text-amber-500" />
                左侧
              </ToggleGroupItem>
              <ToggleGroupItem value="right" class="flex-1">
                <PanelRight class="text-orange-500" />
                右侧
              </ToggleGroupItem>
            </ToggleGroup>
          </Field>
        </FieldGroup>
      </CardContent>
    </Card>

    <Card>
      <CardHeader>
        <CardTitle>显示内容</CardTitle>
        <CardDescription>决定播放器中需要展示的信息和控制项。</CardDescription>
      </CardHeader>
      <CardContent>
        <FieldGroup>
          <Field orientation="horizontal" data-disabled="true">
            <FieldContent>
              <FieldTitle>显示媒体信息</FieldTitle>
              <FieldDescription>显示当前曲目的标题与艺术家。</FieldDescription>
            </FieldContent>
            <Switch aria-label="显示媒体信息" disabled />
          </Field>
          <Field orientation="horizontal" data-disabled="true">
            <FieldContent>
              <FieldTitle>显示播放控制</FieldTitle>
              <FieldDescription>显示上一首、播放和下一首按钮。</FieldDescription>
            </FieldContent>
            <Switch aria-label="显示播放控制" disabled />
          </Field>
        </FieldGroup>
      </CardContent>
    </Card>
  </div>
</template>
