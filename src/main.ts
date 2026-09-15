import { createPinia } from 'pinia'
import { createApp } from 'vue'

import App from './App.vue'
import { i18n, initializeApplicationLocale } from './features/i18n'
import './lib/color-mode'
import router from './router'

import './style.css'

/** 恢复应用级设置后再挂载根组件。 */
async function bootstrap() {
  await initializeApplicationLocale()

  const app = createApp(App)
  app.use(createPinia())
  app.use(router)
  app.use(i18n)
  app.mount('#app')
}

void bootstrap()
