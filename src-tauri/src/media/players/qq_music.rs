use super::PlayerAdapter;
use crate::media::MediaPlayer;

pub(super) static QQ_MUSIC: QqMusicAdapter = QqMusicAdapter;

/// QQ 音乐桌面客户端适配器。
pub(super) struct QqMusicAdapter;

impl PlayerAdapter for QqMusicAdapter {
    fn player(&self) -> MediaPlayer {
        MediaPlayer::QqMusic
    }

    fn matches(&self, source_app_id: &str) -> bool {
        source_app_id.contains("qqmusic")
    }

    fn executable_names(&self) -> &'static [&'static str] {
        &["qqmusic.exe"]
    }
}
