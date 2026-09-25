const automaticInstallRequest = shallowRef(0)

/** 记录一次由原生托盘入口明确触发的全自动安装请求。 */
export function requestAutomaticUpdateInstall(): void {
  automaticInstallRequest.value += 1
}

/** 关于页只读订阅全自动安装请求，实际安装仍由 updater composable 执行。 */
export function useAutomaticUpdateInstallRequest(): Readonly<Ref<number>> {
  return readonly(automaticInstallRequest)
}
