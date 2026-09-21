<script setup lang="ts">
import { CheckIcon, ChevronsUpDownIcon } from '@lucide/vue'

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
import { reportBackgroundFailure } from '@/features/feedback/errors'
import { getCachedSystemFonts, refreshSystemFonts } from '@/features/system/fonts'

interface FontOption {
  value: string
  label: string
}

const SYSTEM_DEFAULT_KEY = '__muse_tune_system_default__'

const props = defineProps<{
  modelValue: string
  disabled: boolean
}>()

const emit = defineEmits<{
  'update:modelValue': [fontFamily: string]
}>()
const { t } = useI18n({ useScope: 'global' })

const systemFonts = shallowRef(getCachedSystemFonts())
const loadFailed = shallowRef(false)

const fontOptions = computed<FontOption[]>(() => {
  const options: FontOption[] = [
    { value: SYSTEM_DEFAULT_KEY, label: t('settings.taskbar.lyrics.systemFont') },
  ]
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
  if (value === SYSTEM_DEFAULT_KEY) return t('settings.taskbar.lyrics.systemFont')
  return typeof value === 'string' ? value : ''
}

/** 后台刷新字体集合；保留旧列表，避免选择器出现加载闪烁。 */
async function refreshFonts() {
  loadFailed.value = false
  try {
    systemFonts.value = await refreshSystemFonts()
  } catch (error) {
    loadFailed.value = true
    reportBackgroundFailure('读取系统字体失败', error)
  }
}

/** 每次展开时后台刷新一次，不使用持续轮询。 */
function refreshFontsWhenOpened(open: boolean) {
  if (open) void refreshFonts()
}

onMounted(refreshFonts)
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
          :aria-label="t('settings.taskbar.lyrics.font')"
        >
          <span class="truncate">{{ selectedFont.label }}</span>
          <ChevronsUpDownIcon data-icon="inline-end" class="opacity-50" />
        </Button>
      </ComboboxTrigger>
    </ComboboxAnchor>

    <ComboboxList class="w-72" align="end">
      <ComboboxInput
        :display-value="displayFontValue"
        :placeholder="t('settings.taskbar.lyrics.searchFonts')"
      />
      <ComboboxViewport class="max-h-72">
        <ComboboxEmpty>
          {{
            loadFailed
              ? t('settings.taskbar.lyrics.fontLoadFailed')
              : t('settings.taskbar.lyrics.noFonts')
          }}
        </ComboboxEmpty>
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
