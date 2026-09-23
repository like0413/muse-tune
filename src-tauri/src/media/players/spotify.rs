use super::PlayerAdapter;
use crate::media::MediaPlayer;

pub(super) static SPOTIFY: SpotifyAdapter = SpotifyAdapter;

/// Spotify Windows 桌面客户端适配器。
pub(super) struct SpotifyAdapter;

impl PlayerAdapter for SpotifyAdapter {
    fn player(&self) -> MediaPlayer {
        MediaPlayer::Spotify
    }

    fn matches(&self, source_app_id: &str) -> bool {
        source_app_id.contains("spotify")
    }

    fn executable_names(&self) -> &'static [&'static str] {
        &["spotify.exe"]
    }

    fn preferred_window_classes(&self) -> &'static [&'static str] {
        &["Chrome_WidgetWin_1"]
    }
}
