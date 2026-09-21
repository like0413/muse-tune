use tauri::{AppHandle, State};

use crate::{
    diagnostics::{self, DiagnosticsSnapshot},
    ipc::IpcError,
    lyrics::LyricsService,
    media::MediaService,
};

/// 按需采集应用、任务栏、媒体与歌词运行状态。
#[tauri::command]
pub async fn collect_diagnostics(
    refresh_storage: Option<bool>,
    app: AppHandle,
    media: State<'_, MediaService>,
    lyrics: State<'_, LyricsService>,
) -> Result<DiagnosticsSnapshot, IpcError> {
    let media = media.inner().clone();
    let lyrics = lyrics.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        diagnostics::collect(&app, &media, &lyrics, refresh_storage.unwrap_or(true))
    })
    .await
    .map_err(|error| {
        IpcError::new(
            "diagnostics.collect",
            format!("等待诊断采集结果失败: {error}"),
            true,
        )
    })
}
