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
        // 网易云同时创建控制台、桌面歌词等顶层窗口，只有宿主窗口应由任务栏唤醒。
        &["OrpheusBrowserHost"]
    }

    fn allows_relaunch_activation(&self) -> bool {
        // BetterNCM 的插件运行时不接受同一客户端入口被再次启动。
        false
    }
}
