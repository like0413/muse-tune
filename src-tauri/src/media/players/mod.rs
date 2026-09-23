//! 播放器适配器只承载各客户端特有的识别和数据修正规则。

mod kugou_music;
mod netease_cloud_music;
mod qq_music;
mod soda_music;
mod spotify;

use std::time::Duration;

use super::MediaPlayer;

/// 隔离单个播放器差异的适配器接口。
trait PlayerAdapter: Sync {
    fn player(&self) -> MediaPlayer;

    /// 判断 GSMTC 来源标识是否属于该播放器。
    fn matches(&self, normalized_source_app_id: &str) -> bool;

    /// 返回传统桌面客户端可能使用的进程文件名。
    fn executable_names(&self) -> &'static [&'static str];

    /// 返回唤醒播放器时优先选择的顶层窗口类名；只影响排序，不放宽唤醒条件。
    fn preferred_window_classes(&self) -> &'static [&'static str] {
        &[]
    }

    /// 返回会使重新启动入口失效的插件模块片段；进程加载了这些模块时改由外部直接显示主窗口。
    fn relaunch_blocking_module_fragments(&self) -> &'static [&'static str] {
        &[]
    }

    /// 返回播放器官方启动入口的文件名；可执行文件自身不是正确入口时需要（例如
    /// 客户端运行在带版本号的子目录、必须由上层启动器转发）。
    fn relaunch_entry_names(&self) -> &'static [&'static str] {
        &[]
    }

    /// 返回歌曲标题变化后保持当前会话的播放器专属稳定窗口。
    fn selection_hold_after_title_change(&self) -> Option<Duration> {
        None
    }
}

static ADAPTERS: [&dyn PlayerAdapter; 5] = [
    &qq_music::QQ_MUSIC,
    &netease_cloud_music::NETEASE_CLOUD_MUSIC,
    &soda_music::SODA_MUSIC,
    &kugou_music::KUGOU_MUSIC,
    &spotify::SPOTIFY,
];

/// 已识别的播放器及其专属适配器。
pub(super) struct IdentifiedPlayer {
    adapter: Option<&'static dyn PlayerAdapter>,
    pub(super) player: MediaPlayer,
}

impl IdentifiedPlayer {
    /// 返回当前播放器隔离维护的进程文件名。
    pub(super) fn executable_names(&self) -> &'static [&'static str] {
        self.adapter.map_or(&[], PlayerAdapter::executable_names)
    }

    pub(super) fn preferred_window_classes(&self) -> &'static [&'static str] {
        self.adapter
            .map_or(&[], PlayerAdapter::preferred_window_classes)
    }

    pub(super) fn relaunch_blocking_module_fragments(&self) -> &'static [&'static str] {
        self.adapter
            .map_or(&[], PlayerAdapter::relaunch_blocking_module_fragments)
    }

    pub(super) fn relaunch_entry_names(&self) -> &'static [&'static str] {
        self.adapter
            .map_or(&[], PlayerAdapter::relaunch_entry_names)
    }
}

/// 根据原始 AUMID 或桌面程序标识选择播放器适配器。
pub(super) fn identify(source_app_id: &str) -> IdentifiedPlayer {
    let normalized = source_app_id.to_ascii_lowercase().replace('\\', "/");
    let adapter = ADAPTERS
        .iter()
        .copied()
        .find(|adapter| adapter.matches(&normalized));

    IdentifiedPlayer {
        player: adapter.map_or(MediaPlayer::Other, PlayerAdapter::player),
        adapter,
    }
}

/// 返回对应播放器在标题变化后的会话稳定窗口。
pub(super) fn selection_hold_after_title_change(player: MediaPlayer) -> Option<Duration> {
    ADAPTERS
        .iter()
        .copied()
        .find(|adapter| adapter.player() == player)
        .and_then(PlayerAdapter::selection_hold_after_title_change)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::media::supported_players;

    /// 适配器表必须覆盖全部已接入播放器：漏掉一个会让该平台的会话完全无法被识别与修正。
    #[test]
    fn adapters_cover_every_supported_player() {
        for player in supported_players() {
            assert!(
                ADAPTERS.iter().any(|adapter| adapter.player() == *player),
                "已接入的 {player:?} 缺少媒体适配器"
            );
        }
    }
}
