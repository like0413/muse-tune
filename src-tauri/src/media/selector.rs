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
    pub(super) selection_held: bool,
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
    if let Some(current) = candidates
        .iter()
        .find(|candidate| Some(candidate.id) == current_id && candidate.selection_held)
    {
        return Some(current.id);
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

#[cfg(test)]
mod tests {
    use super::*;

    /// 构造一个正在播放且元数据完整的候选；测试只覆盖自己关心的字段。
    fn candidate(id: u64, player: MediaPlayer, activity_order: u64) -> SelectionCandidate<'static> {
        SelectionCandidate {
            id,
            player,
            status: MediaPlaybackStatus::Playing,
            activity_order,
            is_windows_current: false,
            title: "同一首歌",
            artist: "同一位歌手",
            has_timeline: true,
            metadata_completeness: 5,
            selection_held: false,
        }
    }

    /// 按指定策略与优先级顺序构造策略，其余字段沿用默认策略。
    fn policy(
        strategy: MediaSessionSelectionStrategy,
        player_priority: Vec<MediaPlayer>,
    ) -> MediaSessionSelectionPolicy {
        MediaSessionSelectionPolicy {
            strategy,
            player_priority,
            ..Default::default()
        }
    }

    /// 没有任何会话时必须返回空选择，否则调用方会保留一个已经消失的控制目标。
    #[test]
    fn no_candidates_select_nothing() {
        let policy = policy(MediaSessionSelectionStrategy::RecentPlayback, Vec::new());

        assert_eq!(select_session(&[], None, &policy), None);
    }

    /// 保持窗口内的当前选择优先：切歌瞬间的短暂空档不能把控制权交给别的播放器。
    #[test]
    fn held_selection_wins_over_recent_playback() {
        let mut held = candidate(1, MediaPlayer::QqMusic, 10);
        held.selection_held = true;
        let recent = candidate(2, MediaPlayer::KugouMusic, 99);
        let policy = policy(MediaSessionSelectionStrategy::RecentPlayback, Vec::new());

        assert_eq!(select_session(&[held, recent], Some(1), &policy), Some(1));
    }

    /// 跟随系统策略下，Windows 当前会话优先于更近的播放活动。
    #[test]
    fn follow_windows_prefers_windows_current_session() {
        let mut windows_current = candidate(1, MediaPlayer::QqMusic, 10);
        windows_current.is_windows_current = true;
        let recent = candidate(2, MediaPlayer::KugouMusic, 99);
        let policy = policy(MediaSessionSelectionStrategy::FollowWindows, Vec::new());

        assert_eq!(
            select_session(&[recent, windows_current], None, &policy),
            Some(1)
        );
    }

    /// 跟随系统策略在缺少 Windows 当前会话时保留原选择，不要跳到别的播放器。
    #[test]
    fn follow_windows_keeps_existing_selection() {
        let mut current = candidate(1, MediaPlayer::QqMusic, 10);
        current.status = MediaPlaybackStatus::Paused;
        let mut other = candidate(2, MediaPlayer::KugouMusic, 99);
        other.status = MediaPlaybackStatus::Paused;
        let policy = policy(MediaSessionSelectionStrategy::FollowWindows, Vec::new());

        assert_eq!(select_session(&[other, current], Some(1), &policy), Some(1));
    }

    /// 最近播放只比较正在播放的会话，暂停中的会话即使活动时间更近也不参与。
    #[test]
    fn recent_playback_ignores_paused_sessions() {
        let playing = candidate(1, MediaPlayer::QqMusic, 10);
        let mut paused = candidate(2, MediaPlayer::KugouMusic, 99);
        paused.status = MediaPlaybackStatus::Paused;
        let policy = policy(MediaSessionSelectionStrategy::RecentPlayback, Vec::new());

        assert_eq!(select_session(&[playing, paused], None, &policy), Some(1));
    }

    /// 多个会话同时播放时取最近进入播放态的那个。
    #[test]
    fn recent_playback_prefers_latest_activity() {
        let older = candidate(1, MediaPlayer::QqMusic, 10);
        let newer = candidate(2, MediaPlayer::KugouMusic, 20);
        let policy = policy(MediaSessionSelectionStrategy::RecentPlayback, Vec::new());

        assert_eq!(select_session(&[older, newer], None, &policy), Some(2));
    }

    /// 锁定当前策略下仍在播放的会话不跟随其他播放器切歌。
    #[test]
    fn sticky_current_keeps_playing_session() {
        let current = candidate(1, MediaPlayer::QqMusic, 10);
        let newer = candidate(2, MediaPlayer::KugouMusic, 99);
        let policy = policy(MediaSessionSelectionStrategy::StickyCurrent, Vec::new());

        assert_eq!(select_session(&[current, newer], Some(1), &policy), Some(1));
    }

    /// 固定优先级严格按配置顺序，不参考播放活动时间。
    #[test]
    fn fixed_priority_follows_configured_order() {
        let just_switched = candidate(1, MediaPlayer::QqMusic, 99);
        let quiet = candidate(2, MediaPlayer::KugouMusic, 10);
        let policy = policy(
            MediaSessionSelectionStrategy::FixedPriority,
            vec![MediaPlayer::KugouMusic, MediaPlayer::QqMusic],
        );

        assert_eq!(
            select_session(&[just_switched, quiet], None, &policy),
            Some(2)
        );
    }

    /// 不在优先级列表里的播放器排在最后，不能因为位置查找失败反而拿到最高优先级。
    #[test]
    fn fixed_priority_ranks_unlisted_players_last() {
        let unlisted = candidate(1, MediaPlayer::SodaMusic, 99);
        let listed = candidate(2, MediaPlayer::KugouMusic, 10);
        let policy = policy(
            MediaSessionSelectionStrategy::FixedPriority,
            vec![MediaPlayer::KugouMusic],
        );

        assert_eq!(select_session(&[unlisted, listed], None, &policy), Some(2));
    }

    /// 同曲目的低质量重复会话不参与竞争，否则元数据更差的那个会抢走控制权。
    #[test]
    fn lower_quality_duplicate_is_excluded() {
        // 同一个播放器的同一首歌：一个没有时间轴但元数据完整，另一个带时间轴但元数据稀少。
        let mut richer_metadata = candidate(2, MediaPlayer::QqMusic, 99);
        richer_metadata.has_timeline = false;
        let mut has_timeline = candidate(1, MediaPlayer::QqMusic, 10);
        has_timeline.metadata_completeness = 1;
        let policy = policy(MediaSessionSelectionStrategy::RecentPlayback, Vec::new());

        assert_eq!(
            select_session(&[richer_metadata, has_timeline], None, &policy),
            Some(1)
        );
    }
}
