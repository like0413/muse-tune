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

    fn preferred_window_classes(&self) -> &'static [&'static str] {
        // QQ 音乐主界面类名与提示、队列等窗口相同，其余辅助窗口由属主、工具窗口和标题过滤排除。
        &["TXGuiFoundation"]
    }
}
