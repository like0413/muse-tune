import { useColorMode } from '@vueuse/core'

/** 当前 WebView 唯一的颜色模式实例，避免多个监听器互相覆盖根元素 class。 */
export const colorMode = useColorMode()
