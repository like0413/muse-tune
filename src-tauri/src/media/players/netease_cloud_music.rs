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
}
