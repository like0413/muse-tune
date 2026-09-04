<script setup lang="ts">
import { Monitor, Moon, Sun } from '@lucide/vue'

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
import { ToggleGroup, ToggleGroupItem } from '@/components/ui/toggle-group'
</script>

<template>
  <Card>
    <CardHeader>
      <CardTitle>主题</CardTitle>
      <CardDescription>调整设置窗口与任务栏播放器的配色。</CardDescription>
      <CardAction>
        <Badge variant="secondary">即将支持</Badge>
      </CardAction>
    </CardHeader>
    <CardContent>
      <FieldGroup>
        <Field data-disabled="true">
          <FieldContent>
            <FieldTitle>界面主题</FieldTitle>
            <FieldDescription>当前跟随 Windows 的系统主题。</FieldDescription>
          </FieldContent>
          <ToggleGroup
            type="single"
            default-value="system"
            variant="outline"
            class="w-full"
            disabled
          >
            <ToggleGroupItem value="light" class="flex-1">
              <Sun class="text-yellow-500" />
              浅色
            </ToggleGroupItem>
            <ToggleGroupItem value="dark" class="flex-1">
              <Moon class="text-indigo-500" />
              深色
            </ToggleGroupItem>
            <ToggleGroupItem value="system" class="flex-1">
              <Monitor class="text-cyan-500" />
              系统
            </ToggleGroupItem>
          </ToggleGroup>
        </Field>
      </FieldGroup>
    </CardContent>
  </Card>
</template>
