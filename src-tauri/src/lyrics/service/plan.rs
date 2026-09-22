use crate::lyrics::model::{LyricsOnlineStrategy, LyricsResolutionSite};
use crate::media::MediaPlayer;

use super::players;

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

/// 同一阶段中的尝试；`parallel` 为真时多个来源并发执行。
pub(super) struct ResolutionStage {
    pub(super) parallel: bool,
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
    /// 歌没有确定关系）；没有对应能力的平台不会产生永不成功的空尝试。
    ///
    /// 两种在线策略与勾选集合的关系，彼此互不影响：
    /// - 仅当前平台：只查当前平台自己的接口，勾选集合被完全忽略；
    /// - 并行：勾选的平台并发执行；当前平台未勾选时隐式补在最前（它就是这首歌最直接的在线
    ///   来源），已勾选时按用户排定的位置，不重复插入。
    pub(super) fn new(
        current_player: MediaPlayer,
        strategy: LyricsOnlineStrategy,
        online_sources: &[MediaPlayer],
    ) -> Self {
        let local_attempts = if players::has_local(current_player) {
            vec![ResolutionAttempt {
                site: LyricsResolutionSite::Local,
                player: current_player,
                capability: ResolutionCapability::Local,
            }]
        } else {
            Vec::new()
        };

        let online_stages = match strategy {
            // 仅当前平台：只查当前平台自己的接口；勾选集合只服务于并行策略，这里完全不看它。
            LyricsOnlineStrategy::CurrentPlayerOnly => {
                if players::has_online(current_player) {
                    vec![ResolutionStage {
                        parallel: false,
                        attempts: vec![ResolutionAttempt {
                            site: LyricsResolutionSite::Online,
                            player: current_player,
                            capability: ResolutionCapability::Online,
                        }],
                    }]
                } else {
                    Vec::new()
                }
            }
            // 并行：当前平台始终参与并排在最前，其余勾选的平台按用户排定的顺序紧随其后，
            // 同一阶段内并发执行。
            LyricsOnlineStrategy::Parallel => {
                let mut planned = online_sources.to_vec();
                // 当前平台是这首歌最直接的在线来源：未勾选时也隐式补在最前；勾选后完全按用户
                // 排定的位置，不重复插入。
                if !online_sources.contains(&current_player) && players::has_online(current_player)
                {
                    planned.insert(0, current_player);
                }
                let attempts = planned
                    .iter()
                    .copied()
                    .filter(|player| players::has_online(*player))
                    .map(|player| ResolutionAttempt {
                        // 站点标识只用于诊断区分“当前平台”与“其他平台”，不影响执行顺序。
                        site: if player == current_player {
                            LyricsResolutionSite::Online
                        } else {
                            LyricsResolutionSite::OnlineFallback
                        },
                        player,
                        capability: ResolutionCapability::Online,
                    })
                    .collect::<Vec<_>>();
                if attempts.is_empty() {
                    Vec::new()
                } else {
                    vec![ResolutionStage {
                        parallel: attempts.len() > 1,
                        attempts,
                    }]
                }
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

    /// 并行策略下发起请求的平台与顺序必须完全等于用户勾选并排定的结果。
    #[test]
    fn parallel_plan_follows_the_configured_sources() {
        let plan = ResolutionPlan::new(
            MediaPlayer::QqMusic,
            LyricsOnlineStrategy::Parallel,
            &[
                MediaPlayer::SodaMusic,
                MediaPlayer::QqMusic,
                MediaPlayer::KugouMusic,
            ],
        );

        assert_eq!(
            sites(&plan.local_attempts),
            vec![(LyricsResolutionSite::Local, MediaPlayer::QqMusic)]
        );
        assert_eq!(plan.online_stages.len(), 1);
        assert!(plan.online_stages[0].parallel);
        assert_eq!(
            sites(&plan.online_stages[0].attempts),
            vec![
                (LyricsResolutionSite::OnlineFallback, MediaPlayer::SodaMusic),
                (LyricsResolutionSite::Online, MediaPlayer::QqMusic),
                (
                    LyricsResolutionSite::OnlineFallback,
                    MediaPlayer::KugouMusic
                ),
            ]
        );
    }

    /// 未勾选的备用平台不能被请求，否则用户关掉某个接口后它仍然会被访问；
    /// 当前平台是唯一例外（见下一个用例）。
    #[test]
    fn parallel_plan_skips_unchecked_sources() {
        let plan = ResolutionPlan::new(
            MediaPlayer::KugouMusic,
            LyricsOnlineStrategy::Parallel,
            &[MediaPlayer::KugouMusic],
        );

        assert_eq!(plan.online_stages.len(), 1);
        assert!(!plan.online_stages[0].parallel);
        assert_eq!(
            sites(&plan.online_stages[0].attempts),
            vec![(LyricsResolutionSite::Online, MediaPlayer::KugouMusic)]
        );
    }

    /// 当前平台的接口未勾选时也要隐式参与并行查询，并排在最前：它就是这首歌最直接的在线来源。
    #[test]
    fn parallel_plan_always_queries_the_current_player_first() {
        let plan = ResolutionPlan::new(
            MediaPlayer::QqMusic,
            LyricsOnlineStrategy::Parallel,
            &[MediaPlayer::KugouMusic, MediaPlayer::NeteaseCloudMusic],
        );

        assert_eq!(plan.online_stages.len(), 1);
        assert!(plan.online_stages[0].parallel);
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

    /// 一个接口都没勾选、当前平台又没有在线能力时不产生在线阶段，
    /// 而不是留下空阶段让调度器空转一轮。
    #[test]
    fn parallel_plan_without_sources_has_no_online_stage() {
        let plan = ResolutionPlan::new(MediaPlayer::Other, LyricsOnlineStrategy::Parallel, &[]);

        assert!(plan.online_stages.is_empty());
    }

    /// 仅当前平台策略只查当前平台自己的在线接口，不引入任何其他平台。
    #[test]
    fn current_player_only_ignores_other_sources() {
        let plan = ResolutionPlan::new(
            MediaPlayer::QqMusic,
            LyricsOnlineStrategy::CurrentPlayerOnly,
            &[
                MediaPlayer::KugouMusic,
                MediaPlayer::QqMusic,
                MediaPlayer::NeteaseCloudMusic,
            ],
        );

        assert_eq!(plan.online_stages.len(), 1);
        assert!(!plan.online_stages[0].parallel);
        assert_eq!(
            sites(&plan.online_stages[0].attempts),
            vec![(LyricsResolutionSite::Online, MediaPlayer::QqMusic)]
        );
    }

    /// 当前平台的接口被取消勾选时，仅当前平台策略依然只查它自己：勾选集合不适用于这个策略。
    #[test]
    fn current_player_only_ignores_the_configured_sources() {
        let plan = ResolutionPlan::new(
            MediaPlayer::QqMusic,
            LyricsOnlineStrategy::CurrentPlayerOnly,
            &[MediaPlayer::KugouMusic],
        );

        assert_eq!(plan.online_stages.len(), 1);
        assert!(!plan.online_stages[0].parallel);
        assert_eq!(
            sites(&plan.online_stages[0].attempts),
            vec![(LyricsResolutionSite::Online, MediaPlayer::QqMusic)]
        );
    }

    /// 当前平台没有在线能力时，仅当前平台策略不产生空阶段。
    #[test]
    fn current_player_only_without_online_capability_has_no_online_stage() {
        let plan = ResolutionPlan::new(
            MediaPlayer::Other,
            LyricsOnlineStrategy::CurrentPlayerOnly,
            &[],
        );

        assert!(plan.online_stages.is_empty());
    }

    /// 汽水音乐没有本地歌词能力，不能为它生成一条永远返回 Unsupported 的尝试。
    #[test]
    fn player_without_local_capability_has_no_local_attempt() {
        let plan = ResolutionPlan::new(
            MediaPlayer::SodaMusic,
            LyricsOnlineStrategy::Parallel,
            &[MediaPlayer::SodaMusic],
        );

        assert!(plan.local_attempts.is_empty());
        assert_eq!(
            sites(&plan.online_stages[0].attempts),
            vec![(LyricsResolutionSite::Online, MediaPlayer::SodaMusic)]
        );
    }

    /// 未识别播放器没有任何适配器，勾选全部平台时只能靠其他平台兜底。
    #[test]
    fn unsupported_player_uses_every_configured_source() {
        let plan = ResolutionPlan::new(
            MediaPlayer::Other,
            LyricsOnlineStrategy::Parallel,
            crate::media::supported_players(),
        );

        assert!(plan.local_attempts.is_empty());
        assert_eq!(
            sites(&plan.online_stages[0].attempts),
            crate::media::supported_players()
                .iter()
                .map(|player| (LyricsResolutionSite::OnlineFallback, *player))
                .collect::<Vec<_>>()
        );
    }

    /// 全部勾选时每个具备在线能力的平台都必须生成尝试：漏掉一个会让该平台的在线源永远不被请求。
    #[test]
    fn parallel_plan_covers_every_online_capable_player() {
        let sources = crate::media::supported_players();
        let online_capable = sources
            .iter()
            .filter(|player| players::has_online(**player))
            .count();
        let plan = ResolutionPlan::new(
            MediaPlayer::QqMusic,
            LyricsOnlineStrategy::Parallel,
            sources,
        );

        assert_eq!(plan.online_stages[0].attempts.len(), online_capable);
    }
}
