import messages from '@intlify/unplugin-vue-i18n/messages'
import { useColorMode } from '@vueuse/core'
import { createPinia } from 'pinia'
import { createApp } from 'vue'
import { createI18n } from 'vue-i18n'

import App from './App.vue'
import router from './router'

import './style.css'

// 在应用挂载前恢复颜色模式，默认跟随 Windows，并同步系统主题变化。
useColorMode()

const i18n = createI18n({
  locale: 'zh',
  fallbackLocale: 'zh',
  messages,
})

const app = createApp(App)

app.use(createPinia())
app.use(router)
app.use(i18n)

app.mount('#app')
