use super::{
    MediaPlayer, MediaSessionSelectionPolicy, MediaSessionSelectionStrategy,
    model::MediaPlaybackStatus,
};

/// 与 WinRT 无关的会话选择输入，便于独立演进抢占规则。
pub(super) struct SelectionCandidate<'a> {
    pub(super) id: u64,
    pub(super) player: MediaPlayer,
    pub(super) status: MediaPlaybackStatus,
    pub(super) activity_order: u64,
    pub(super) is_windows_current: bool,
    pub(super) title: &'a str,
    pub(super) artist: &'a str,
    pub(super) has_timeline: bool,
    pub(super) metadata_completeness: u8,
}

impl SelectionCandidate<'_> {
    /// 当前会话是否明确处于播放状态。
    fn is_playing(&self) -> bool {
        matches!(self.status, MediaPlaybackStatus::Playing)
    }

    /// 返回同曲目重复会话的质量排序键，时间轴优先于补充元数据。
    fn quality(&self) -> (bool, u8) {
        (self.has_timeline, self.metadata_completeness)
    }

    /// 判断两个候选是否来自同一播放器并描述同一首歌曲。
    fn is_same_media(&self, other: &Self) -> bool {
        self.player == other.player
            && !self.title.is_empty()
            && self.title == other.title
            && self.artist == other.artist
    }
}

/// 按用户策略从全部有效会话中选出唯一控制目标。
pub(super) fn select_session(
    candidates: &[SelectionCandidate<'_>],
    current_id: Option<u64>,
    policy: &MediaSessionSelectionPolicy,
) -> Option<u64> {
    if candidates.is_empty() {
        return None;
    }

    match policy.strategy {
        MediaSessionSelectionStrategy::FollowWindows => windows_current(candidates)
            .or_else(|| existing_current(candidates, current_id))
            .or_else(|| first_preferred(candidates)),
        MediaSessionSelectionStrategy::RecentPlayback => recent_playing(candidates)
            .or_else(|| existing_current(candidates, current_id))
            .or_else(|| windows_current(candidates))
            .or_else(|| first_preferred(candidates)),
        MediaSessionSelectionStrategy::StickyCurrent => {
            let current_playing = candidates
                .iter()
                .find(|candidate| {
                    Some(candidate.id) == current_id
                        && candidate.is_playing()
                        && !is_lower_quality_duplicate(candidate, candidates)
                })
                .map(|candidate| candidate.id);
            current_playing
                .or_else(|| recent_playing(candidates))
                .or_else(|| existing_current(candidates, current_id))
                .or_else(|| windows_current(candidates))
                .or_else(|| first_preferred(candidates))
        }
        MediaSessionSelectionStrategy::FixedPriority => fixed_priority(candidates, policy)
            .or_else(|| existing_current(candidates, current_id))
            .or_else(|| windows_current(candidates))
            .or_else(|| first_preferred(candidates)),
    }
}

/// 返回 Windows 当前会话。
fn windows_current(candidates: &[SelectionCandidate<'_>]) -> Option<u64> {
    candidates
        .iter()
        .find(|candidate| {
            candidate.is_windows_current && !is_lower_quality_duplicate(candidate, candidates)
        })
        .map(|candidate| candidate.id)
}

/// 保留仍存在的当前选择。
fn existing_current(candidates: &[SelectionCandidate<'_>], current_id: Option<u64>) -> Option<u64> {
    candidates
        .iter()
        .find(|candidate| {
            Some(candidate.id) == current_id && !is_lower_quality_duplicate(candidate, candidates)
        })
        .map(|candidate| candidate.id)
}

/// 选择最近从非播放态进入播放态的会话。
fn recent_playing(candidates: &[SelectionCandidate<'_>]) -> Option<u64> {
    candidates
        .iter()
        .filter(|candidate| {
            candidate.is_playing() && !is_lower_quality_duplicate(candidate, candidates)
        })
        .max_by_key(|candidate| (candidate.activity_order, candidate.is_windows_current))
        .map(|candidate| candidate.id)
}

/// 在所有正在播放的会话中应用用户指定的播放器优先级。
fn fixed_priority(
    candidates: &[SelectionCandidate<'_>],
    policy: &MediaSessionSelectionPolicy,
) -> Option<u64> {
    candidates
        .iter()
        .filter(|candidate| {
            candidate.is_playing() && !is_lower_quality_duplicate(candidate, candidates)
        })
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

/// 返回未被同曲目更高质量会话替代的首个候选。
fn first_preferred(candidates: &[SelectionCandidate<'_>]) -> Option<u64> {
    candidates
        .iter()
        .find(|candidate| !is_lower_quality_duplicate(candidate, candidates))
        .map(|candidate| candidate.id)
}

/// 判断候选是否存在同曲目且质量更高的重复会话。
fn is_lower_quality_duplicate(
    candidate: &SelectionCandidate<'_>,
    candidates: &[SelectionCandidate<'_>],
) -> bool {
    candidates.iter().any(|other| {
        other.id != candidate.id
            && candidate.is_same_media(other)
            && other.quality() > candidate.quality()
    })
}
