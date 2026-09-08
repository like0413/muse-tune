use super::PlayerAdapter;
use crate::media::MediaPlayer;

pub(super) static SODA_MUSIC: SodaMusicAdapter = SodaMusicAdapter;

/// 汽水音乐桌面客户端适配器。
pub(super) struct SodaMusicAdapter;

impl PlayerAdapter for SodaMusicAdapter {
    fn player(&self) -> MediaPlayer {
        MediaPlayer::SodaMusic
    }

    fn matches(&self, source_app_id: &str) -> bool {
        source_app_id.contains("汽水音乐")
            || source_app_id.contains("sodamusic")
            || source_app_id.contains("soda.music")
            || source_app_id.contains("qishui")
    }

    fn executable_names(&self) -> &'static [&'static str] {
        &["sodamusic.exe", "qishui.exe"]
    }
}
