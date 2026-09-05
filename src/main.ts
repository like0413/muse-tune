import messages from '@intlify/unplugin-vue-i18n/messages'
import { createPinia } from 'pinia'
import { createApp } from 'vue'
import { createI18n } from 'vue-i18n'

import App from './App.vue'
import router from './router'
import './lib/color-mode'

import './style.css'

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
