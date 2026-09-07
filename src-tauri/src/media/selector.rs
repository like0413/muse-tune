use super::{
    MediaPlayer, MediaSessionSelectionPolicy, MediaSessionSelectionStrategy,
    model::MediaPlaybackStatus,
};

/// 与 WinRT 无关的会话选择输入，便于独立演进抢占规则。
pub(super) struct SelectionCandidate {
    pub(super) id: u64,
    pub(super) player: MediaPlayer,
    pub(super) status: MediaPlaybackStatus,
    pub(super) activity_order: u64,
    pub(super) is_windows_current: bool,
}

impl SelectionCandidate {
    /// 当前会话是否明确处于播放状态。
    fn is_playing(&self) -> bool {
        matches!(self.status, MediaPlaybackStatus::Playing)
    }
}

/// 按用户策略从全部有效会话中选出唯一控制目标。
pub(super) fn select_session(
    candidates: &[SelectionCandidate],
    current_id: Option<u64>,
    policy: &MediaSessionSelectionPolicy,
) -> Option<u64> {
    if candidates.is_empty() {
        return None;
    }

    match policy.strategy {
        MediaSessionSelectionStrategy::FollowWindows => windows_current(candidates)
            .or_else(|| existing_current(candidates, current_id))
            .or_else(|| candidates.first().map(|candidate| candidate.id)),
        MediaSessionSelectionStrategy::RecentPlayback => recent_playing(candidates)
            .or_else(|| existing_current(candidates, current_id))
            .or_else(|| windows_current(candidates))
            .or_else(|| candidates.first().map(|candidate| candidate.id)),
        MediaSessionSelectionStrategy::StickyCurrent => {
            let current_playing = candidates
                .iter()
                .find(|candidate| Some(candidate.id) == current_id && candidate.is_playing())
                .map(|candidate| candidate.id);
            current_playing
                .or_else(|| recent_playing(candidates))
                .or_else(|| existing_current(candidates, current_id))
                .or_else(|| windows_current(candidates))
                .or_else(|| candidates.first().map(|candidate| candidate.id))
        }
        MediaSessionSelectionStrategy::FixedPriority => fixed_priority(candidates, policy)
            .or_else(|| existing_current(candidates, current_id))
            .or_else(|| windows_current(candidates))
            .or_else(|| candidates.first().map(|candidate| candidate.id)),
    }
}

/// 返回 Windows 当前会话。
fn windows_current(candidates: &[SelectionCandidate]) -> Option<u64> {
    candidates
        .iter()
        .find(|candidate| candidate.is_windows_current)
        .map(|candidate| candidate.id)
}

/// 保留仍存在的当前选择。
fn existing_current(candidates: &[SelectionCandidate], current_id: Option<u64>) -> Option<u64> {
    candidates
        .iter()
        .find(|candidate| Some(candidate.id) == current_id)
        .map(|candidate| candidate.id)
}

/// 选择最近从非播放态进入播放态的会话。
fn recent_playing(candidates: &[SelectionCandidate]) -> Option<u64> {
    candidates
        .iter()
        .filter(|candidate| candidate.is_playing())
        .max_by_key(|candidate| (candidate.activity_order, candidate.is_windows_current))
        .map(|candidate| candidate.id)
}

/// 在所有正在播放的会话中应用用户指定的播放器优先级。
fn fixed_priority(
    candidates: &[SelectionCandidate],
    policy: &MediaSessionSelectionPolicy,
) -> Option<u64> {
    candidates
        .iter()
        .filter(|candidate| candidate.is_playing())
        .min_by_key(|candidate| {
            let priority = policy
                .player_priority
                .iter()
                .position(|player| *player == candidate.player)
                .unwrap_or(usize::MAX);
            (priority, std::cmp::Reverse(candidate.activity_order))
        })
        .map(|candidate| candidate.id)
}
