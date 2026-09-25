<script setup lang="ts">
import { ItemGroup } from '@/components/ui/item'
import { useAutomaticUpdateInstallRequest } from '@/features/updater/install-intent'
import { useApplicationUpdater } from '@/features/updater/useApplicationUpdater'

import ProjectInfoItem from './ProjectInfoItem.vue'
import UpdateItem from './UpdateItem.vue'

const {
  status,
  statusLabel,
  isChecking,
  isDownloading,
  availableUpdate,
  detectedVersion,
  automaticCheck,
  automaticCheckSaving,
  updateCheckFrequency,
  updateCheckFrequencySaving,
  downloadProgress,
  errorMessage,
  checkForUpdates,
  openReleaseNotes,
  installUpdate,
  installUpdateAutomatically,
  updateAutomaticCheck,
  updateAutomaticCheckFrequency,
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
  <ItemGroup class="gap-3">
    <UpdateItem
      :status="status"
      :status-label="statusLabel"
      :is-checking="isChecking"
      :is-downloading="isDownloading"
      :update="availableUpdate"
      :detected-version="detectedVersion"
      :automatic-check="automaticCheck"
      :automatic-check-saving="automaticCheckSaving"
      :update-check-frequency="updateCheckFrequency"
      :update-check-frequency-saving="updateCheckFrequencySaving"
      :download-progress="downloadProgress"
      :error-message="errorMessage"
      @check="checkForUpdates"
      @open-release-notes="openReleaseNotes"
      @install="installUpdate"
      @update-automatic-check="updateAutomaticCheck"
      @update-automatic-check-frequency="updateAutomaticCheckFrequency"
    />
    <ProjectInfoItem />
  </ItemGroup>
</template>
