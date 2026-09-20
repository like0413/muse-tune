use crate::media::MediaPlayer;

use super::players;
use crate::lyrics::model::{LyricsOnlineStrategy, LyricsParallelGroup, LyricsResolutionSite};

/// 在线歌词兜底来源的尝试顺序；当前播放器会被排除，其余按此顺序补齐。
///
/// 顺序集中在这里：原先逐个 `if current_player != ...` 写死，新增平台或调整优先级需要在多处
/// 同步修改，容易漏掉某个平台或让它在不同阶段以不同顺序出现。
const ONLINE_FALLBACK_ORDER: [MediaPlayer; 3] = [
    MediaPlayer::QqMusic,
    MediaPlayer::KugouMusic,
    MediaPlayer::NeteaseCloudMusic,
];

/// 单次歌词来源尝试所调用的能力。
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum ResolutionCapability {
    Local,
    Online,
}

/// 一次可诊断的歌词来源尝试。
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct ResolutionAttempt {
    pub(super) site: LyricsResolutionSite,
    pub(super) player: MediaPlayer,
    pub(super) capability: ResolutionCapability,
}

/// 同一阶段中的尝试；多于一个来源时允许并行执行。
pub(super) struct ResolutionStage {
    pub(super) group: Option<LyricsParallelGroup>,
    pub(super) attempts: Vec<ResolutionAttempt>,
}

/// 把产品层的来源优先级编译为可执行计划。
pub(super) struct ResolutionPlan {
    pub(super) local_attempts: Vec<ResolutionAttempt>,
    pub(super) online_stages: Vec<ResolutionStage>,
}

impl ResolutionPlan {
    /// 编译一次解析计划。
    ///
    /// 只生成真正能执行的尝试：本地歌词只查当前播放器（其他播放器的本地缓存与正在播放的这首
    /// 歌没有确定关系），在线来源则按固定优先级补齐平台，并跳过当前播放器与没有在线能力的平台。
    pub(super) fn new(current_player: MediaPlayer, strategy: LyricsOnlineStrategy) -> Self {
        let local_attempts = if players::has_local(current_player) {
            vec![ResolutionAttempt {
                site: LyricsResolutionSite::Local,
                player: current_player,
                capability: ResolutionCapability::Local,
            }]
        } else {
            Vec::new()
        };

        let current = players::has_online(current_player).then_some(ResolutionAttempt {
            site: match strategy {
                LyricsOnlineStrategy::Parallel => LyricsResolutionSite::Online,
                LyricsOnlineStrategy::CurrentPlayerFirst => LyricsResolutionSite::OnlinePreferred,
            },
            player: current_player,
            capability: ResolutionCapability::Online,
        });
        let fallbacks = ONLINE_FALLBACK_ORDER
            .into_iter()
            .filter(|player| *player != current_player && players::has_online(*player))
            .map(|player| ResolutionAttempt {
                site: LyricsResolutionSite::OnlineFallback,
                player,
                capability: ResolutionCapability::Online,
            })
            .collect::<Vec<_>>();

        let online_stages = match strategy {
            LyricsOnlineStrategy::Parallel => {
                let attempts = current.into_iter().chain(fallbacks).collect::<Vec<_>>();
                if attempts.is_empty() {
                    Vec::new()
                } else {
                    vec![ResolutionStage {
                        group: (attempts.len() > 1).then_some(LyricsParallelGroup::Online),
                        attempts,
                    }]
                }
            }
            LyricsOnlineStrategy::CurrentPlayerFirst => {
                let mut stages = Vec::with_capacity(2);
                if let Some(current) = current {
                    stages.push(ResolutionStage {
                        group: None,
                        attempts: vec![current],
                    });
                }
                if !fallbacks.is_empty() {
                    stages.push(ResolutionStage {
                        group: (fallbacks.len() > 1).then_some(LyricsParallelGroup::OnlineFallback),
                        attempts: fallbacks,
                    });
                }
                stages
            }
        };

        Self {
            local_attempts,
            online_stages,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 只保留站点顺序，便于直接断言计划形状。
    fn sites(attempts: &[ResolutionAttempt]) -> Vec<(LyricsResolutionSite, MediaPlayer)> {
        attempts
            .iter()
            .map(|attempt| (attempt.site, attempt.player))
            .collect()
    }

    /// 并发策略把当前播放器与全部兜底来源放进同一阶段。
    #[test]
    fn parallel_plan_merges_online_sources_into_one_stage() {
        let plan = ResolutionPlan::new(MediaPlayer::QqMusic, LyricsOnlineStrategy::Parallel);

        assert_eq!(
            sites(&plan.local_attempts),
            vec![(LyricsResolutionSite::Local, MediaPlayer::QqMusic)]
        );
        assert_eq!(plan.online_stages.len(), 1);
        assert_eq!(
            plan.online_stages[0].group,
            Some(LyricsParallelGroup::Online)
        );
        assert_eq!(
            sites(&plan.online_stages[0].attempts),
            vec![
                (LyricsResolutionSite::Online, MediaPlayer::QqMusic),
                (
                    LyricsResolutionSite::OnlineFallback,
                    MediaPlayer::KugouMusic
                ),
                (
                    LyricsResolutionSite::OnlineFallback,
                    MediaPlayer::NeteaseCloudMusic
                ),
            ]
        );
    }

    /// "当前播放器优先"必须把当前播放器单独放第一阶段，否则并行就失去了优先级含义。
    #[test]
    fn current_player_first_splits_online_into_two_stages() {
        let plan = ResolutionPlan::new(
            MediaPlayer::QqMusic,
            LyricsOnlineStrategy::CurrentPlayerFirst,
        );

        assert_eq!(plan.online_stages.len(), 2);
        assert_eq!(plan.online_stages[0].group, None);
        assert_eq!(
            sites(&plan.online_stages[0].attempts),
            vec![(LyricsResolutionSite::OnlinePreferred, MediaPlayer::QqMusic)]
        );
        assert_eq!(
            plan.online_stages[1].group,
            Some(LyricsParallelGroup::OnlineFallback)
        );
        assert_eq!(
            sites(&plan.online_stages[1].attempts),
            vec![
                (
                    LyricsResolutionSite::OnlineFallback,
                    MediaPlayer::KugouMusic
                ),
                (
                    LyricsResolutionSite::OnlineFallback,
                    MediaPlayer::NeteaseCloudMusic
                ),
            ]
        );
    }

    /// 汽水音乐没有本地歌词能力，不能为它生成一条永远返回 Unsupported 的尝试。
    #[test]
    fn player_without_local_capability_has_no_local_attempt() {
        let plan = ResolutionPlan::new(MediaPlayer::SodaMusic, LyricsOnlineStrategy::Parallel);

        assert!(plan.local_attempts.is_empty());
        assert_eq!(plan.online_stages.len(), 1);
        assert_eq!(
            sites(&plan.online_stages[0].attempts),
            vec![
                (LyricsResolutionSite::Online, MediaPlayer::SodaMusic),
                (LyricsResolutionSite::OnlineFallback, MediaPlayer::QqMusic),
                (
                    LyricsResolutionSite::OnlineFallback,
                    MediaPlayer::KugouMusic
                ),
                (
                    LyricsResolutionSite::OnlineFallback,
                    MediaPlayer::NeteaseCloudMusic
                ),
            ]
        );
    }

    /// 未识别播放器没有任何适配器，只能靠其他平台兜底。
    #[test]
    fn unsupported_player_falls_back_to_all_online_sources() {
        let plan = ResolutionPlan::new(MediaPlayer::Other, LyricsOnlineStrategy::Parallel);

        assert!(plan.local_attempts.is_empty());
        assert_eq!(plan.online_stages.len(), 1);
        assert_eq!(
            sites(&plan.online_stages[0].attempts),
            vec![
                (LyricsResolutionSite::OnlineFallback, MediaPlayer::QqMusic),
                (
                    LyricsResolutionSite::OnlineFallback,
                    MediaPlayer::KugouMusic
                ),
                (
                    LyricsResolutionSite::OnlineFallback,
                    MediaPlayer::NeteaseCloudMusic
                ),
            ]
        );
    }

    /// 当前播放器没有在线能力时，第一阶段不该存在——空阶段会让调度器空转一轮。
    #[test]
    fn current_player_first_without_online_capability_yields_only_fallback_stage() {
        let plan =
            ResolutionPlan::new(MediaPlayer::Other, LyricsOnlineStrategy::CurrentPlayerFirst);

        assert_eq!(plan.online_stages.len(), 1);
        assert_eq!(
            plan.online_stages[0].group,
            Some(LyricsParallelGroup::OnlineFallback)
        );
        assert_eq!(
            sites(&plan.online_stages[0].attempts),
            vec![
                (LyricsResolutionSite::OnlineFallback, MediaPlayer::QqMusic),
                (
                    LyricsResolutionSite::OnlineFallback,
                    MediaPlayer::KugouMusic
                ),
                (
                    LyricsResolutionSite::OnlineFallback,
                    MediaPlayer::NeteaseCloudMusic
                ),
            ]
        );
    }
}
