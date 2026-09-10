<script setup lang="ts">
import { CheckIcon, ChevronsUpDownIcon, LoaderCircleIcon } from '@lucide/vue'
import { computed, shallowRef } from 'vue'

import { Button } from '@/components/ui/button'
import {
  Combobox,
  ComboboxAnchor,
  ComboboxEmpty,
  ComboboxGroup,
  ComboboxInput,
  ComboboxItem,
  ComboboxItemIndicator,
  ComboboxList,
  ComboboxTrigger,
  ComboboxViewport,
} from '@/components/ui/combobox'
import { listSystemFonts } from '@/features/system/fonts'

interface FontOption {
  value: string
  label: string
}

const SYSTEM_DEFAULT_KEY = '__muse_tune_system_default__'
const SYSTEM_DEFAULT_LABEL = '系统默认'

const props = defineProps<{
  modelValue: string
  disabled: boolean
}>()

const emit = defineEmits<{
  'update:modelValue': [fontFamily: string]
}>()

const systemFonts = shallowRef<readonly string[]>([])
const loading = shallowRef(false)
const loadFailed = shallowRef(false)

const fontOptions = computed<FontOption[]>(() => {
  const options: FontOption[] = [{ value: SYSTEM_DEFAULT_KEY, label: SYSTEM_DEFAULT_LABEL }]
  if (props.modelValue && !systemFonts.value.includes(props.modelValue)) {
    options.push({ value: props.modelValue, label: props.modelValue })
  }
  return options.concat(systemFonts.value.map((font) => ({ value: font, label: font })))
})

const selectedFont = computed(
  () =>
    fontOptions.value.find((option) => option.value === props.modelValue) ?? fontOptions.value[0]!,
)
const selectedFontKey = computed(() => props.modelValue || SYSTEM_DEFAULT_KEY)

/** 使用稳定字符串键提交字体；专用占位值转换为空字符串以继承任务栏原字体。 */
function selectFont(value: unknown) {
  if (typeof value !== 'string') return
  emit('update:modelValue', value === SYSTEM_DEFAULT_KEY ? '' : value)
}

/** 避免内部占位键出现在可编辑搜索框中。 */
function displayFontValue(value: unknown): string {
  if (value === SYSTEM_DEFAULT_KEY) return SYSTEM_DEFAULT_LABEL
  return typeof value === 'string' ? value : ''
}

/** 下拉框每次打开时刷新字体集合，使新安装字体无需重启应用即可出现。 */
async function loadFonts() {
  if (loading.value) return
  loading.value = true
  loadFailed.value = false
  try {
    systemFonts.value = await listSystemFonts()
  } catch (error) {
    loadFailed.value = true
    console.error('读取系统字体失败', error)
  } finally {
    loading.value = false
  }
}

/** 仅响应用户打开动作，不在任务栏或设置页后台轮询字体。 */
function refreshFontsWhenOpened(open: boolean) {
  if (open) void loadFonts()
}
</script>

<template>
  <Combobox
    :model-value="selectedFontKey"
    @update:model-value="selectFont"
    @update:open="refreshFontsWhenOpened"
  >
    <ComboboxAnchor as-child>
      <ComboboxTrigger as-child>
        <Button
          variant="outline"
          class="w-56 justify-between font-normal"
          :disabled="disabled"
          role="combobox"
          aria-label="歌词字体"
        >
          <span class="truncate">{{ selectedFont.label }}</span>
          <LoaderCircleIcon v-if="loading" data-icon="inline-end" class="animate-spin" />
          <ChevronsUpDownIcon v-else data-icon="inline-end" class="opacity-50" />
        </Button>
      </ComboboxTrigger>
    </ComboboxAnchor>

    <ComboboxList class="w-72" align="end">
      <ComboboxInput :display-value="displayFontValue" placeholder="搜索系统字体…" />
      <ComboboxViewport class="max-h-72">
        <ComboboxEmpty>{{ loadFailed ? '系统字体读取失败' : '未找到字体' }}</ComboboxEmpty>
        <ComboboxGroup>
          <ComboboxItem
            v-for="font in fontOptions"
            :key="font.value"
            :value="font.value"
            :text-value="font.label"
          >
            <span class="truncate">{{ font.label }}</span>
            <ComboboxItemIndicator>
              <CheckIcon />
            </ComboboxItemIndicator>
          </ComboboxItem>
        </ComboboxGroup>
      </ComboboxViewport>
    </ComboboxList>
  </Combobox>
</template>
