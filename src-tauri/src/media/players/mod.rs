//! 播放器适配器只承载各客户端特有的识别和数据修正规则。

mod kugou_music;
mod netease_cloud_music;
mod qq_music;
mod soda_music;

use super::MediaPlayer;

/// 隔离单个播放器差异的适配器接口。
trait PlayerAdapter: Sync {
    /// 返回稳定的播放器标识。
    fn player(&self) -> MediaPlayer;

    /// 判断 GSMTC 来源标识是否属于该播放器。
    fn matches(&self, normalized_source_app_id: &str) -> bool;

    /// 返回传统桌面客户端可能使用的进程文件名。
    fn executable_names(&self) -> &'static [&'static str];
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
