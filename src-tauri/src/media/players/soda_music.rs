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

    fn preferred_window_classes(&self) -> &'static [&'static str] {
        // 汽水音乐的 Electron 主窗口用可带标题的 Widget 类；无标题的同名宿主窗口由标题过滤排除。
        &["Chrome_WidgetWin_1"]
    }

    fn relaunch_entry_names(&self) -> &'static [&'static str] {
        // 主窗口由 Chromium 托管，隐藏后不能由外部显示（界面会卡死），只能走官方入口；
        // 运行中的客户端位于带版本号的子目录，官方入口是安装根目录下的启动器
        // （等于开始菜单快捷方式的目标）。
        &["SodaMusicLauncher.exe"]
    }
}
