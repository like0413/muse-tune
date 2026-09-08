import { createRouter, createWebHashHistory } from 'vue-router'

import SettingsPage from '@/pages/settings/index.vue'
import TaskbarPage from '@/pages/taskbar/index.vue'

const router = createRouter({
  history: createWebHashHistory(import.meta.env.BASE_URL),
  routes: [
    {
      path: '/taskbar',
      name: 'taskbar',
      component: TaskbarPage,
    },
    {
      path: '/settings',
      name: 'settings',
      component: SettingsPage,
    },
    {
      path: '/volume',
      name: 'volume',
      component: () => import('@/pages/volume/index.vue'),
    },
  ],
})

export default router
