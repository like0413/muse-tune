use tauri::{AppHandle, State};

use crate::{
    data::{self, DataDirectoryKind, DataOverview},
    error::Error,
    ipc::IpcError,
    logging,
    lyrics::LyricsService,
};

/// 读取数据页所需的缓存、配置和日志摘要。
#[tauri::command]
pub async fn get_data_overview(
    app: AppHandle,
    lyrics: State<'_, LyricsService>,
) -> Result<DataOverview, IpcError> {
    let lyrics = lyrics.inner().clone();
    tauri::async_runtime::spawn_blocking(move || data::overview(&app, &lyrics))
        .await
        .map_err(|error| {
            IpcError::new(
                "data.get-overview",
                format!("等待数据统计结果失败: {error}"),
                true,
            )
        })?
        .map_err(|error| IpcError::new("data.get-overview", error, true))
}

/// 打开由应用路径解析器确定的数据目录。
#[tauri::command]
pub async fn open_data_directory(
    kind: DataDirectoryKind,
    app: AppHandle,
    lyrics: State<'_, LyricsService>,
) -> Result<(), IpcError> {
    let lyrics = lyrics.inner().clone();
    tauri::async_runtime::spawn_blocking(move || data::open_directory(&app, &lyrics, kind))
        .await
        .map_err(|error| {
            IpcError::new(
                "data.open-directory",
                format!("等待目录打开结果失败: {error}"),
                true,
            )
        })?
        .map_err(|error| IpcError::new("data.open-directory", error, true))
}

/// 恢复默认配置后重启应用：设置只在启动时加载一次，不重启会让已运行的窗口继续沿用旧值。
#[tauri::command]
pub async fn reset_configuration(app: AppHandle) -> Result<(), IpcError> {
    let reset_app = app.clone();
    tauri::async_runtime::spawn_blocking(move || data::reset_configuration(&reset_app))
        .await
        .map_err(|error| {
            IpcError::new(
                "data.reset-configuration",
                format!("等待配置重置结果失败: {error}"),
                false,
            )
        })?
        .map_err(|error| IpcError::new("data.reset-configuration", error, false))?;
    app.request_restart();
    Ok(())
}

/// 清空应用歌词缓存，并返回清理后的最新容量状态。
#[tauri::command]
pub async fn clear_lyrics_cache(
    app: AppHandle,
    lyrics: State<'_, LyricsService>,
) -> Result<DataOverview, IpcError> {
    let lyrics = lyrics.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        lyrics
            .clear_cache()
            .map_err(|error| Error::Message(format!("清理缓存失败: {error}")))?;
        data::overview(&app, &lyrics)
    })
    .await
    .map_err(|error| {
        IpcError::new(
            "data.clear-lyrics-cache",
            format!("等待缓存清理结果失败: {error}"),
            true,
        )
    })?
    .map_err(|error| IpcError::new("data.clear-lyrics-cache", error, true))
}

/// 只清理当前歌曲缓存，并返回最新容量状态。
#[tauri::command]
pub async fn clear_current_lyrics_cache(
    app: AppHandle,
    lyrics: State<'_, LyricsService>,
) -> Result<DataOverview, IpcError> {
    let lyrics = lyrics.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        lyrics.clear_current_cache()?;
        data::overview(&app, &lyrics)
    })
    .await
    .map_err(|error| {
        IpcError::new(
            "data.clear-current-lyrics-cache",
            format!("等待当前歌曲缓存清理结果失败: {error}"),
            true,
        )
    })?
    .map_err(|error| IpcError::new("data.clear-current-lyrics-cache", error, true))
}

/// 删除当前歌曲缓存并强制重新执行歌词解析链路。
#[tauri::command]
pub async fn refresh_current_lyrics(
    app: AppHandle,
    lyrics: State<'_, LyricsService>,
) -> Result<DataOverview, IpcError> {
    let lyrics = lyrics.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        lyrics.refresh_current()?;
        data::overview(&app, &lyrics)
    })
    .await
    .map_err(|error| {
        IpcError::new(
            "data.refresh-current-lyrics",
            format!("等待当前歌词重新获取失败: {error}"),
            true,
        )
    })?
    .map_err(|error| IpcError::new("data.refresh-current-lyrics", error, true))
}

/// 清空轮转历史日志、保留活动日志，并返回最新容量状态。
#[tauri::command]
pub async fn clear_log_history(
    app: AppHandle,
    lyrics: State<'_, LyricsService>,
) -> Result<DataOverview, IpcError> {
    let lyrics = lyrics.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        logging::clear_history(&app)?;
        data::overview(&app, &lyrics)
    })
    .await
    .map_err(|error| {
        IpcError::new(
            "data.clear-log-history",
            format!("等待历史日志清理结果失败: {error}"),
            true,
        )
    })?
    .map_err(|error| IpcError::new("data.clear-log-history", error, true))
}
