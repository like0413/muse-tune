use std::path::PathBuf;

use tauri::{AppHandle, Runtime, State};
use tauri_plugin_store::StoreExt;

use crate::{
    lyrics::{LyricsCachePathState, LyricsService, LyricsSnapshot},
    media::MediaPlayer,
    settings_store::PATH as SETTINGS_STORE_PATH,
};

/// 返回最近一次歌词解析状态，供新创建的任务栏窗口补取初始值。
#[tauri::command]
pub fn get_current_lyrics(service: State<'_, LyricsService>) -> LyricsSnapshot {
    service.snapshot()
}

/// 返回四家播放器当前的自动发现、手动覆盖和实际使用目录。
#[tauri::command]
pub fn get_lyrics_cache_paths(service: State<'_, LyricsService>) -> Vec<LyricsCachePathState> {
    service.cache_paths()
}

/// 同步歌词总开关到后端解析生命周期。
#[tauri::command]
pub fn set_lyrics_enabled(enabled: bool, service: State<'_, LyricsService>) -> Result<(), String> {
    service.set_enabled(enabled)
}

/// 保存或清除指定播放器的歌词缓存目录覆盖，并立即重新解析当前歌曲。
#[tauri::command]
pub fn set_lyrics_cache_path_override<R: Runtime>(
    player: MediaPlayer,
    path: Option<String>,
    app: AppHandle<R>,
    service: State<'_, LyricsService>,
) -> Result<Vec<LyricsCachePathState>, String> {
    if player == MediaPlayer::Other {
        return Err("不支持为未识别播放器设置歌词目录".to_owned());
    }

    let normalized = path
        .map(PathBuf::from)
        .map(|path| {
            path.canonicalize()
                .map_err(|error| format!("歌词目录不可用: {error}"))
        })
        .transpose()?;
    if normalized.as_ref().is_some_and(|path| !path.is_dir()) {
        return Err("歌词缓存路径必须是目录".to_owned());
    }

    let store = app
        .store(SETTINGS_STORE_PATH)
        .map_err(|error| format!("打开设置存储失败: {error}"))?;
    let key = crate::lyrics::settings::override_store_key(player)
        .ok_or_else(|| "不支持的播放器".to_owned())?;
    if let Some(path) = &normalized {
        store.set(
            key,
            serde_json::Value::String(path.to_string_lossy().into_owned()),
        );
    } else {
        store.delete(key);
    }
    store
        .save()
        .map_err(|error| format!("保存歌词目录设置失败: {error}"))?;

    service.set_cache_path_override(player, normalized)?;
    Ok(service.cache_paths())
}
