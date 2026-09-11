//! 播放器适配器只承载各客户端特有的识别和数据修正规则。

mod kugou_music;
mod netease_cloud_music;
mod qq_music;
mod soda_music;

use std::time::Duration;

use super::MediaPlayer;

/// 隔离单个播放器差异的适配器接口。
trait PlayerAdapter: Sync {
    /// 返回稳定的播放器标识。
    fn player(&self) -> MediaPlayer;

    /// 判断 GSMTC 来源标识是否属于该播放器。
    fn matches(&self, normalized_source_app_id: &str) -> bool;

    /// 返回传统桌面客户端可能使用的进程文件名。
    fn executable_names(&self) -> &'static [&'static str];

    /// 返回激活播放器时优先选择的顶层窗口类名。
    fn preferred_window_classes(&self) -> &'static [&'static str] {
        &[]
    }

    /// 指示隐藏窗口无法恢复时，是否允许再次启动客户端入口进行唤醒。
    fn allows_relaunch_activation(&self) -> bool {
        true
    }

    /// 返回歌曲标题变化后保持当前会话的播放器专属稳定窗口。
    fn selection_hold_after_title_change(&self) -> Option<Duration> {
        None
    }
}

static ADAPTERS: [&dyn PlayerAdapter; 4] = [
    &qq_music::QQ_MUSIC,
    &netease_cloud_music::NETEASE_CLOUD_MUSIC,
    &soda_music::SODA_MUSIC,
    &kugou_music::KUGOU_MUSIC,
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

    /// 返回当前播放器主窗口的稳定类名。
    pub(super) fn preferred_window_classes(&self) -> &'static [&'static str] {
        self.adapter
            .map_or(&[], PlayerAdapter::preferred_window_classes)
    }

    /// 返回播放器是否支持通过单实例入口安全唤醒。
    pub(super) fn allows_relaunch_activation(&self) -> bool {
        self.adapter
            .is_none_or(PlayerAdapter::allows_relaunch_activation)
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
