use tauri::{Manager, Runtime};
use tauri_plugin_store::StoreExt;

use crate::media::MediaPlayer;
use crate::native_defaults::{self, LyricsChineseVariantPreference, LyricsNetworkPolicy};
use crate::storage::StoragePaths;

use super::model::{LyricsChineseVariant, LyricsOnlineStrategy};

const LYRICS_DISPLAY_KEY: &str = "taskbar.lyrics";
const APPLICATION_LOCALE_KEY: &str = "application.locale";

/// 需要作为一个快照提交和读取的歌词运行偏好。
#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct LyricsPreferences {
    pub(super) enabled: bool,
    pub(super) chinese_variant: LyricsChineseVariant,
    pub(super) allow_online: bool,
    pub(super) online_strategy: LyricsOnlineStrategy,
    /// 参与在线检索的平台，顺序即尝试顺序；只含用户勾选过的平台。
    pub(super) online_sources: Vec<MediaPlayer>,
}

impl Default for LyricsPreferences {
    /// 状态不可用时回退到与前端共用同一份数据的默认偏好。
    fn default() -> Self {
        let defaults = &native_defaults::shared().taskbar.lyrics;
        Self {
            enabled: defaults.enabled,
            // 应用默认语言是简体中文；存储不可用时仍与前端默认行为一致。
            chinese_variant: defaults.chinese_variant.resolve("zh-Hans"),
            allow_online: defaults.network_policy.allows_online(),
            online_strategy: defaults.online_strategy,
            online_sources: defaults.online_sources.clone(),
        }
    }
}

/// 从版本化前端设置中恢复歌词运行偏好，各字段独立回退共享默认值。
///
/// 存储结构是 `{version, value}` 包装，这里必须与前端一样在版本不符时判废旧值：前端的
/// `loadVersionedSetting` 会丢弃过期版本并回写默认值，若这里照用旧值，整个会话内原生与
/// 前端就会按不同偏好运行。版本号与默认值同来自 `native-defaults.json`。
///
/// 枚举字段按取值严格解析，非法值回退默认值，与前端 `normalizeTaskbarLyricsSettings` 一致。
pub(super) fn restore_lyrics_preferences<R: Runtime>(app: &tauri::App<R>) -> LyricsPreferences {
    let shared = native_defaults::shared();
    let defaults = &shared.taskbar.lyrics;
    let store = app.store(app.state::<StoragePaths>().settings_file()).ok();
    let value = store
        .as_ref()
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
    let chinese_variant_preference = value
        .get("chineseVariant")
        .and_then(|variant| {
            serde_json::from_value::<LyricsChineseVariantPreference>(variant.clone()).ok()
        })
        .unwrap_or(defaults.chinese_variant);
    let locale = store
        .as_ref()
        .and_then(|store| store.get(APPLICATION_LOCALE_KEY))
        .and_then(|locale| locale.as_str().map(str::to_owned))
        .unwrap_or_else(|| "zh-Hans".to_owned());
    let allow_online = value
        .get("networkPolicy")
        .and_then(|policy| serde_json::from_value::<LyricsNetworkPolicy>(policy.clone()).ok())
        .unwrap_or(defaults.network_policy)
        .allows_online();
    let online_strategy = value
        .get("onlineStrategy")
        .and_then(|strategy| serde_json::from_value::<LyricsOnlineStrategy>(strategy.clone()).ok())
        .unwrap_or(defaults.online_strategy);
    let online_sources = restore_online_sources(&value);
    LyricsPreferences {
        enabled,
        chinese_variant: chinese_variant_preference.resolve(&locale),
        allow_online,
        online_strategy,
        online_sources,
    }
}

/// 由持久化的“完整顺序 + 启用集合”解析出实际参与检索的平台。
///
/// 规则与前端 `normalizeTaskbarLyricsSettings` 保持一致：顺序先补齐为全部在线平台各一次，
/// 再与启用集合求交并保持顺序；任一项缺失或损坏时按“全部平台都启用”回退。
fn restore_online_sources(value: &serde_json::Value) -> Vec<MediaPlayer> {
    let supported = native_defaults::shared()
        .taskbar
        .lyrics
        .online_sources
        .as_slice();
    let order = value
        .get("onlineSourceOrder")
        .and_then(|order| serde_json::from_value::<Vec<MediaPlayer>>(order.clone()).ok())
        .map(|order| complete_platform_order(&order, supported))
        .unwrap_or_else(|| supported.to_vec());
    let enabled = value
        .get("enabledOnlineSources")
        .and_then(|enabled| serde_json::from_value::<Vec<MediaPlayer>>(enabled.clone()).ok())
        .map(|enabled| dedupe_supported(&enabled, supported))
        .unwrap_or_else(|| order.clone());
    order
        .into_iter()
        .filter(|player| enabled.contains(player))
        .collect()
}

/// 把用户排定的顺序补齐成“全部已接入平台各一次”的完整排列。
fn complete_platform_order(order: &[MediaPlayer], supported: &[MediaPlayer]) -> Vec<MediaPlayer> {
    let mut completed = dedupe_supported(order, supported);
    for player in supported {
        if !completed.contains(player) {
            completed.push(*player);
        }
    }
    completed
}

/// 去重并剔除未接入的平台。
fn dedupe_supported(players: &[MediaPlayer], supported: &[MediaPlayer]) -> Vec<MediaPlayer> {
    let mut deduped = Vec::with_capacity(players.len());
    for player in players {
        if supported.contains(player) && !deduped.contains(player) {
            deduped.push(*player);
        }
    }
    deduped
}
