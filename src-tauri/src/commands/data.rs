use tauri::{AppHandle, State};

use crate::{
    data::{self, DataDirectoryKind, DataOverview},
    lyrics::LyricsService,
};

/// 读取数据页所需的缓存、配置和日志摘要。
#[tauri::command]
pub async fn get_data_overview(
    app: AppHandle,
    lyrics: State<'_, LyricsService>,
) -> Result<DataOverview, String> {
    let lyrics = lyrics.inner().clone();
    tauri::async_runtime::spawn_blocking(move || data::overview(&app, &lyrics))
        .await
        .map_err(|error| format!("等待数据统计结果失败: {error}"))?
}

/// 打开由应用路径解析器确定的数据目录。
#[tauri::command]
pub async fn open_data_directory(
    kind: DataDirectoryKind,
    app: AppHandle,
    lyrics: State<'_, LyricsService>,
) -> Result<(), String> {
    let lyrics = lyrics.inner().clone();
    tauri::async_runtime::spawn_blocking(move || data::open_directory(&app, &lyrics, kind))
        .await
        .map_err(|error| format!("等待目录打开结果失败: {error}"))?
}

/// 恢复默认配置，并通过 Tauri 的正常退出流程重启应用。
#[tauri::command]
pub async fn reset_configuration(app: AppHandle) -> Result<(), String> {
    let reset_app = app.clone();
    tauri::async_runtime::spawn_blocking(move || data::reset_configuration(&reset_app))
        .await
        .map_err(|error| format!("等待配置重置结果失败: {error}"))??;
    app.request_restart();
    Ok(())
}

/// 清空应用歌词缓存，并返回清理后的最新容量状态。
#[tauri::command]
pub async fn clear_lyrics_cache(
    app: AppHandle,
    lyrics: State<'_, LyricsService>,
) -> Result<DataOverview, String> {
    let lyrics = lyrics.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        lyrics
            .clear_cache()
            .map_err(|error| format!("清理缓存失败: {error}"))?;
        data::overview(&app, &lyrics)
    })
    .await
    .map_err(|error| format!("等待缓存清理结果失败: {error}"))?
}

/// 清空日志文件，并重启应用以重新建立日志写入句柄。
#[tauri::command]
pub async fn clear_logs(app: AppHandle) -> Result<(), String> {
    let clear_app = app.clone();
    tauri::async_runtime::spawn_blocking(move || data::clear_logs(&clear_app))
        .await
        .map_err(|error| format!("等待日志清理结果失败: {error}"))??;
    app.request_restart();
    Ok(())
}
