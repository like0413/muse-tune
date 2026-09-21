//! 前端与原生共用的启动期默认值与存储版本。
//!
//! 数据源是 `src/features/settings/native-defaults.json`，由 `include_str!` 在编译期嵌入，
//! 运行时不依赖该文件；前端 `features/settings/defaults.ts` 与
//! `features/settings/storage/schema-versions.ts` 读取同一份数据。这些取值同时被原生启动期
//! 读取和前端使用，因此两端不再各自维护字面量。
//!
//! 取值非法时 `shared()` 会直接失败。该文件是编译期契约，字段名或枚举取值写错属于开发期错误，
//! 应当在首次运行时立即暴露，而不是静默回退到另一份重复的兜底字面量。

use std::sync::LazyLock;

use crate::lyrics::LyricsOnlineStrategy;
use crate::media::{MediaPlayer, MediaSessionSelectionStrategy};
use crate::taskbar::{TaskbarOverlapPriority, TaskbarPlacement, TaskbarWidthMode};

const SHARED_DEFAULTS: &str = include_str!("../../src/features/settings/native-defaults.json");

/// `taskbar.lyrics.networkPolicy` 的取值，与前端持久化格式一致。
#[derive(Clone, Copy, Debug, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LyricsNetworkPolicy {
    Auto,
    LocalOnly,
}

impl LyricsNetworkPolicy {
    /// 仅“只使用本地歌词”会禁止在线解析。
    pub const fn allows_online(self) -> bool {
        matches!(self, Self::Auto)
    }
}

/// 前端与原生共用的默认值。
#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SharedDefaults {
    pub taskbar: TaskbarDefaults,
    pub media: MediaDefaults,
    /// 原生启动期需要自行判废的版本化设置；未在此声明的版本条目会被 serde 忽略。
    pub versions: Versions,
}

/// 原生需要自行校验存储版本的设置分组。
#[derive(Debug, serde::Deserialize)]
pub struct Versions {
    pub taskbar: TaskbarVersions,
}

/// 任务栏下的版本化设置；目前只有歌词偏好需要原生在启动期判废。
#[derive(Debug, serde::Deserialize)]
pub struct TaskbarVersions {
    pub lyrics: u64,
}

/// 原生窗口创建前需要读取的任务栏设置。
#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskbarDefaults {
    pub width_min: i32,
    pub width_max: i32,
    pub width: i32,
    pub width_mode: TaskbarWidthMode,
    pub placement: TaskbarPlacement,
    pub overlap_priority: TaskbarOverlapPriority,
    pub display_target: String,
    pub lyrics: LyricsDefaults,
}

/// 歌词服务的启动偏好。
#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LyricsDefaults {
    pub enabled: bool,
    pub network_policy: LyricsNetworkPolicy,
    pub online_strategy: LyricsOnlineStrategy,
}

/// 媒体服务在前端推送到达前使用的会话选择策略。
#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MediaDefaults {
    pub selection_strategy: MediaSessionSelectionStrategy,
    pub only_supported_players: bool,
    /// 已接入播放器及其规范顺序；前端与原生共用这一份列表。
    pub supported_players: Vec<MediaPlayer>,
}

static DEFAULTS: LazyLock<SharedDefaults> = LazyLock::new(|| {
    serde_json::from_str(SHARED_DEFAULTS)
        .expect("src/features/settings/native-defaults.json 无法解析，请检查字段名与枚举取值")
});

/// 返回共享默认值，首次调用时解析并缓存。
pub fn shared() -> &'static SharedDefaults {
    &DEFAULTS
}

/// 目标显示器哨兵值：在所有任务栏上显示。
pub fn display_target() -> &'static str {
    &shared().taskbar.display_target
}

/// 已接入的播放器及其规范顺序。
///
/// 这是“哪些平台已接入、默认如何排序”的唯一事实源：默认会话策略、策略归一化与候选择优
/// 权重都从这里派生。用户调整优先级只影响持久化策略，不会改变这份规范顺序。
pub fn supported_players() -> &'static [MediaPlayer] {
    &shared().media.supported_players
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 共享文件是编译期嵌入的契约，字段名或枚举取值写错不应当推迟到运行时才暴露。
    #[test]
    fn shared_defaults_parse() {
        let taskbar = &shared().taskbar;
        assert!(
            taskbar.width_min <= taskbar.width && taskbar.width <= taskbar.width_max,
            "默认宽度必须落在可调范围内，否则会被原生钳制而与设置界面不一致"
        );
    }

    /// 共享平台列表是前端规范化与原生归一化的共同来源：损坏或重复会让平台被错误地排除。
    #[test]
    fn supported_players_are_well_formed() {
        let players = supported_players();
        assert!(!players.is_empty(), "共享配置里的已接入平台列表不能为空");

        let mut seen = Vec::new();
        for player in players {
            assert_ne!(
                *player,
                MediaPlayer::Other,
                "Other 不是已接入平台，不该出现在列表里"
            );
            assert!(
                !seen.contains(player),
                "{player:?} 在共享平台列表里重复出现"
            );
            seen.push(*player);
        }
    }
}
