use tauri::Runtime;
use tauri_plugin_store::StoreExt;

use crate::settings_store::PATH as SETTINGS_STORE_PATH;

use super::model::LyricsOnlineStrategy;

const LYRICS_DISPLAY_KEY: &str = "taskbar.lyrics";

/// 从版本化前端设置中恢复歌词运行偏好，损坏字段独立使用默认值。
pub fn restore_lyrics_preferences<R: Runtime>(
    app: &tauri::App<R>,
) -> (bool, bool, LyricsOnlineStrategy) {
    let value = app
        .store(SETTINGS_STORE_PATH)
        .ok()
        .and_then(|store| store.get(LYRICS_DISPLAY_KEY))
        .and_then(|stored| stored.get("value").cloned())
        .unwrap_or_default();
    let enabled = value
        .get("enabled")
        .and_then(serde_json::Value::as_bool)
        .unwrap_or(true);
    let allow_online = value
        .get("networkPolicy")
        .and_then(serde_json::Value::as_str)
        .is_none_or(|policy| policy != "local_only");
    let online_strategy = match value
        .get("onlineStrategy")
        .and_then(serde_json::Value::as_str)
    {
        Some("current_player_first") => LyricsOnlineStrategy::CurrentPlayerFirst,
        _ => LyricsOnlineStrategy::Parallel,
    };
    (enabled, allow_online, online_strategy)
}
