<script setup lang="ts">
import { useAutomaticUpdateInstallRequest } from '@/features/updater/install-intent'
import { useApplicationUpdater } from '@/features/updater/useApplicationUpdater'

import ProjectFeedback from './ProjectFeedback.vue'
import ProjectOverview from './ProjectOverview.vue'
import UpdateItem from './UpdateItem.vue'

defineProps<{ applicationVersion: string }>()

const {
  status,
  statusLabel,
  isChecking,
  isDownloading,
  availableUpdate,
  detectedVersion,
  downloadProgress,
  errorMessage,
  isPortable,
  checkForUpdates,
  installUpdate,
  installUpdateAutomatically,
} = useApplicationUpdater()

const automaticInstallRequest = useAutomaticUpdateInstallRequest()
let handledAutomaticInstallRequest = 0

/** 每次托盘点击只消费一次；重复事件由 updater composable 的安装锁继续兜底。 */
watch(
  automaticInstallRequest,
  (request) => {
    if (request <= handledAutomaticInstallRequest) return
    handledAutomaticInstallRequest = request
    void installUpdateAutomatically()
  },
  { immediate: true },
)
</script>

<template>
  <div class="flex w-full flex-col gap-3">
    <ProjectOverview :application-version="applicationVersion" />
    <UpdateItem
      :status="status"
      :status-label="statusLabel"
      :is-checking="isChecking"
      :is-downloading="isDownloading"
      :update="availableUpdate"
      :detected-version="detectedVersion"
      :download-progress="downloadProgress"
      :error-message="errorMessage"
      :is-portable="isPortable"
      @check="checkForUpdates"
      @install="installUpdate"
    />
    <ProjectFeedback />
  </div>
</template>
