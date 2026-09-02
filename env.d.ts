/// <reference types="vite/client" />

declare module '*.vue' {
  import type { DefineComponent } from 'vue'
  // oxlint-disable-next-line typescript/no-empty-object-type
  const component: DefineComponent<{}, {}, unknown>
  export default component
}
