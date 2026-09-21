use std::time::Duration;

use super::PlayerAdapter;
use crate::media::MediaPlayer;

/// 标题变化后的会话保持窗口：窗口内择优会锁定当前会话，不让其他播放器更近的播放活动抢走控制目标。
/// 调小会让切歌瞬间的控制目标过于敏感地跳走，调大会延后跟随用户真正切到的播放器。
const TRACK_CHANGE_SELECTION_HOLD: Duration = Duration::from_millis(400);

pub(super) static KUGOU_MUSIC: KugouMusicAdapter = KugouMusicAdapter;

/// 酷狗音乐桌面客户端适配器。
pub(super) struct KugouMusicAdapter;

impl PlayerAdapter for KugouMusicAdapter {
    fn player(&self) -> MediaPlayer {
        MediaPlayer::KugouMusic
    }

    fn matches(&self, source_app_id: &str) -> bool {
        source_app_id.contains("kugou") || source_app_id.contains("kgmusic")
    }

    fn executable_names(&self) -> &'static [&'static str] {
        &["kugou.exe", "kgmusic.exe"]
    }

    fn preferred_window_classes(&self) -> &'static [&'static str] {
        // 酷狗的界面窗口共用类名；排除的是同进程的隐藏宿主窗口，主窗口再按标题与面积区分。
        &["kugou_ui"]
    }

    fn selection_hold_after_title_change(&self) -> Option<Duration> {
        Some(TRACK_CHANGE_SELECTION_HOLD)
    }
}
