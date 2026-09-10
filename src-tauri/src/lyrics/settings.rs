use tauri::Runtime;
use tauri_plugin_store::StoreExt;

use crate::settings_store::PATH as SETTINGS_STORE_PATH;

const LYRICS_DISPLAY_KEY: &str = "taskbar.lyrics";

/// 从版本化前端设置中恢复歌词总开关，损坏数据保持默认开启。
pub fn restore_lyrics_enabled<R: Runtime>(app: &tauri::App<R>) -> bool {
    app.store(SETTINGS_STORE_PATH)
        .ok()
        .and_then(|store| store.get(LYRICS_DISPLAY_KEY))
        .and_then(|stored| stored.get("value").cloned())
        .and_then(|value| value.get("enabled").and_then(serde_json::Value::as_bool))
        .unwrap_or(true)
}
