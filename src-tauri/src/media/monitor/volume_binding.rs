//! 把选中的 GSMTC 来源绑定到对应播放器的 Windows 应用音频会话。
//!
//! Core Audio 的会话往往晚于 GSMTC 就绪（播放器刚启动时还没有音频流），
//! 因此这里带一个有限次退避重试，而不是一次性绑定失败就放弃。

use tauri::{AppHandle, Runtime};

use crate::media::{players::identify, volume::ApplicationVolumeController};

use super::{SelectedMedia, SessionEntry, deadlines::WorkerDeadlines, publisher::publish_volume};

/// 按已选 GSMTC 来源绑定对应播放器的 Windows 应用音频会话。
pub(super) fn bind_selected_volume(
    volume: &mut ApplicationVolumeController,
    deadlines: &mut WorkerDeadlines,
    entries: &[SessionEntry],
    selected_id: Option<u64>,
) {
    let Some(entry) = entries.iter().find(|entry| Some(entry.id) == selected_id) else {
        volume.bind(None, "", &[]);
        deadlines.cancel_volume_rebind();
        return;
    };
    let source_app_id = entry
        .registration
        .session
        .SourceAppUserModelId()
        .map(|value| value.to_string())
        .unwrap_or_default();
    let player = identify(&source_app_id);
    volume.bind(selected_id, &source_app_id, player.executable_names());
    if volume.snapshot().is_none() {
        deadlines.schedule_volume_rebind(entry.id, 0);
    } else {
        deadlines.cancel_volume_rebind();
    }
}

/// Core Audio 通知会话集合变化后重新匹配当前播放器进程。
pub(super) fn rebind_selected_volume(
    volume: &mut ApplicationVolumeController,
    entries: &[SessionEntry],
    target_id: u64,
) {
    let Some(entry) = entries.iter().find(|entry| entry.id == target_id) else {
        return;
    };
    let source_app_id = entry
        .registration
        .session
        .SourceAppUserModelId()
        .map(|value| value.to_string())
        .unwrap_or_default();
    let player = identify(&source_app_id);
    volume.rebind(target_id, &source_app_id, player.executable_names());
}

/// 执行一次到期的音量重绑，并仅在仍未找到音频会话时安排下一档退避。
pub(super) fn handle_volume_rebind_due<R: Runtime>(
    app: &AppHandle<R>,
    sessions: &[SessionEntry],
    selected: &mut SelectedMedia<R>,
    deadlines: &mut WorkerDeadlines,
    target_id: u64,
    attempt: usize,
) {
    if selected.id != Some(target_id) || selected.volume.snapshot().is_some() {
        return;
    }
    rebind_selected_volume(&mut selected.volume, sessions, target_id);
    selected.spectrum.bind(selected.volume.capture_process_id());
    let volume = selected.volume.snapshot();
    publish_volume(app, volume);
    if volume.is_none() {
        if !deadlines.schedule_volume_rebind(target_id, attempt.saturating_add(1)) {
            // 退避用尽仍未绑上，音量控制会一直是空的；重试过程本身不值得记，结论必须记。
            log::warn!("绑定播放器应用音量失败：重试已用尽，仍未找到当前播放器的音频会话");
        }
    } else {
        deadlines.cancel_volume_rebind();
    }
}
