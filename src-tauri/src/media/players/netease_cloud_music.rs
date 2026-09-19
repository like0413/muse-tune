use super::PlayerAdapter;
use crate::media::MediaPlayer;

pub(super) static NETEASE_CLOUD_MUSIC: NeteaseCloudMusicAdapter = NeteaseCloudMusicAdapter;

/// 网易云音乐桌面客户端适配器。
pub(super) struct NeteaseCloudMusicAdapter;

impl PlayerAdapter for NeteaseCloudMusicAdapter {
    fn player(&self) -> MediaPlayer {
        MediaPlayer::NeteaseCloudMusic
    }

    fn matches(&self, source_app_id: &str) -> bool {
        source_app_id.contains("cloudmusic") || source_app_id.contains("netease")
    }

    fn executable_names(&self) -> &'static [&'static str] {
        &["cloudmusic.exe"]
    }

    fn preferred_window_classes(&self) -> &'static [&'static str] {
        // 网易云同时创建桌面歌词、迷你播放器、控制台宿主等顶层窗口，只有该宿主窗口是主界面。
        &["OrpheusBrowserHost"]
    }

    fn relaunch_blocking_module_fragments(&self) -> &'static [&'static str] {
        // BetterNCM 以插件运行时注入客户端，路径与模块名都可能变化，按片段匹配完整模块路径。
        &["betterncm"]
    }
}
