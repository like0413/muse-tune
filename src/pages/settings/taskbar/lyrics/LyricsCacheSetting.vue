<script setup lang="ts">
import { FolderOpen, Languages, RotateCcw } from '@lucide/vue'
import type { UnlistenFn } from '@tauri-apps/api/event'
import { listen } from '@tauri-apps/api/event'
import { open } from '@tauri-apps/plugin-dialog'
import { onMounted, shallowRef } from 'vue'

import CollapsibleItem from '@/components/settings/CollapsibleItem.vue'
import { Badge } from '@/components/ui/badge'
import { Button } from '@/components/ui/button'
import {
  Field,
  FieldContent,
  FieldDescription,
  FieldGroup,
  FieldTitle,
} from '@/components/ui/field'
import { Input } from '@/components/ui/input'
import { ItemContent, ItemDescription, ItemMedia, ItemTitle } from '@/components/ui/item'
import { Spinner } from '@/components/ui/spinner'
import { getLyricsCachePaths, setLyricsCachePathOverride } from '@/features/lyrics/cache-paths'
import type { LyricsCachePathState } from '@/features/lyrics/types'
import type { MediaPlayer } from '@/features/media/types'

const playerLabels: Record<Exclude<MediaPlayer, 'other'>, string> = {
  qq_music: 'QQ 音乐',
  netease_cloud_music: '网易云音乐',
  soda_music: '汽水音乐',
  kugou_music: '酷狗音乐',
}

const paths = shallowRef<LyricsCachePathState[]>([])
const loading = shallowRef(false)
const savingPlayer = shallowRef<MediaPlayer | null>(null)
let unlistenPathsChange: UnlistenFn | undefined

/** 读取后端实际使用的目录，避免设置页自行猜测播放器路径。 */
async function loadPaths() {
  loading.value = true
  try {
    paths.value = await getLyricsCachePaths()
  } catch (error) {
    console.error('读取歌词缓存目录失败', error)
  } finally {
    loading.value = false
  }
}

/** 使用官方原生目录选择器保存单个平台覆盖。 */
async function chooseDirectory(state: LyricsCachePathState) {
  if (savingPlayer.value) return
  try {
    const selected = await open({
      directory: true,
      multiple: false,
      defaultPath: state.effectivePath ?? undefined,
      title: `选择${playerLabel(state.player)}歌词缓存目录`,
    })
    if (typeof selected !== 'string') return
    savingPlayer.value = state.player
    paths.value = await setLyricsCachePathOverride(state.player, selected)
  } catch (error) {
    console.error('设置歌词缓存目录失败', error)
  } finally {
    savingPlayer.value = null
  }
}

/** 清除手动目录并恢复读取播放器自身配置。 */
async function restoreAutomatic(state: LyricsCachePathState) {
  if (savingPlayer.value || !state.overridePath) return
  savingPlayer.value = state.player
  try {
    paths.value = await setLyricsCachePathOverride(state.player, null)
  } catch (error) {
    console.error('恢复歌词目录自动发现失败', error)
  } finally {
    savingPlayer.value = null
  }
}

function playerLabel(player: MediaPlayer): string {
  return player === 'other' ? '未知播放器' : playerLabels[player]
}

/** 订阅播放器自身配置变化，使设置页与后端路径事实源保持一致。 */
async function initialize() {
  try {
    unlistenPathsChange = await listen<LyricsCachePathState[]>(
      'lyrics://cache-paths-changed',
      ({ payload }) => {
        paths.value = payload
      },
    )
  } catch (error) {
    console.error('监听歌词缓存目录失败', error)
  }
  await loadPaths()
}

onMounted(initialize)
onUnmounted(() => unlistenPathsChange?.())
</script>

<template>
  <CollapsibleItem>
    <ItemMedia class="icon-tone-violet-500">
      <Languages />
    </ItemMedia>
    <ItemContent>
      <ItemTitle>歌词缓存目录</ItemTitle>
      <ItemDescription>优先读取播放器本地缓存，也可为每家播放器手动覆盖</ItemDescription>
    </ItemContent>

    <template #content>
      <div v-if="loading" class="text-muted-foreground flex items-center gap-2 text-sm">
        <Spinner />
        正在检测播放器目录
      </div>
      <FieldGroup v-else>
        <Field
          v-for="state in paths"
          :key="state.player"
          orientation="responsive"
          :data-invalid="Boolean(state.effectivePath) && !state.exists"
        >
          <FieldContent>
            <div class="flex items-center gap-2">
              <FieldTitle>{{ playerLabel(state.player) }}</FieldTitle>
              <Badge :variant="state.exists ? 'secondary' : 'outline'">
                {{ state.overridePath ? '手动覆盖' : state.exists ? '已自动发现' : '未发现' }}
              </Badge>
            </div>
            <FieldDescription>
              {{ state.overridePath ? '当前使用手动目录' : '当前跟随播放器配置自动发现' }}
            </FieldDescription>
            <Input
              :model-value="state.effectivePath ?? '未找到可用目录'"
              readonly
              :aria-invalid="Boolean(state.effectivePath) && !state.exists"
              :aria-label="`${playerLabel(state.player)}歌词缓存目录`"
            />
          </FieldContent>
          <div class="flex shrink-0 items-center gap-2">
            <Button
              variant="outline"
              size="sm"
              :disabled="savingPlayer !== null"
              @click="chooseDirectory(state)"
            >
              <Spinner v-if="savingPlayer === state.player" data-icon="inline-start" />
              <FolderOpen v-else data-icon="inline-start" />
              选择目录
            </Button>
            <Button
              variant="ghost"
              size="sm"
              :disabled="savingPlayer !== null || !state.overridePath"
              @click="restoreAutomatic(state)"
            >
              <RotateCcw data-icon="inline-start" />
              恢复自动
            </Button>
          </div>
        </Field>
      </FieldGroup>
    </template>
  </CollapsibleItem>
</template>
