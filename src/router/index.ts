import { createRouter, createWebHashHistory } from 'vue-router'

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
      component: () => import('@/pages/settings/index.vue'),
    },
  ],
})

export default router
