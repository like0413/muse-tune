use tauri::Runtime;
use tauri_plugin_store::StoreExt;

use crate::native_defaults::{self, LyricsNetworkPolicy};
use crate::settings_store::PATH as SETTINGS_STORE_PATH;

use super::model::LyricsOnlineStrategy;

const LYRICS_DISPLAY_KEY: &str = "taskbar.lyrics";

/// 从版本化前端设置中恢复歌词运行偏好，各字段独立回退共享默认值。
///
/// 存储结构是 `{version, value}` 包装，这里必须与前端一样在版本不符时判废旧值：前端的
/// `loadVersionedSetting` 会丢弃过期版本并回写默认值，若这里照用旧值，整个会话内原生与
/// 前端就会按不同偏好运行。版本号与默认值同来自 `native-defaults.json`。
///
/// 枚举字段按取值严格解析，非法值回退默认值，与前端 `normalizeTaskbarLyricsSettings` 一致。
pub fn restore_lyrics_preferences<R: Runtime>(
    app: &tauri::App<R>,
) -> (bool, bool, LyricsOnlineStrategy) {
    let shared = native_defaults::shared();
    let defaults = &shared.taskbar.lyrics;
    let value = app
        .store(SETTINGS_STORE_PATH)
        .ok()
        .and_then(|store| store.get(LYRICS_DISPLAY_KEY))
        .filter(|stored| {
            stored.get("version").and_then(serde_json::Value::as_u64)
                == Some(shared.versions.taskbar.lyrics)
        })
        .and_then(|stored| stored.get("value").cloned())
        .unwrap_or_default();
    let enabled = value
        .get("enabled")
        .and_then(serde_json::Value::as_bool)
        .unwrap_or(defaults.enabled);
    let allow_online = value
        .get("networkPolicy")
        .and_then(|policy| serde_json::from_value::<LyricsNetworkPolicy>(policy.clone()).ok())
        .unwrap_or(defaults.network_policy)
        .allows_online();
    let online_strategy = value
        .get("onlineStrategy")
        .and_then(|strategy| serde_json::from_value::<LyricsOnlineStrategy>(strategy.clone()).ok())
        .unwrap_or(defaults.online_strategy);
    (enabled, allow_online, online_strategy)
}
