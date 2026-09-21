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
    /// 歌没有确定关系）；在线来源严格按用户勾选并排定的顺序生成，未勾选的平台不会被请求，
    /// 没有在线能力的平台也不会产生永不成功的空尝试。
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
            // 仅当前平台：不引入任何其他平台的来源；当前平台的接口没勾选时完全不发起在线检索。
            LyricsOnlineStrategy::CurrentPlayerOnly => {
                if online_sources.contains(&current_player) && players::has_online(current_player) {
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
            // 并行：勾选的平台放在同一阶段并发执行，阶段内顺序即用户排定的请求顺序。
            LyricsOnlineStrategy::Parallel => {
                let attempts = online_sources
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

    /// 未勾选的平台不能被请求，否则用户关掉某个接口后它仍然会被访问。
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

    /// 一个接口都没勾选时不产生在线阶段，而不是留下空阶段让调度器空转一轮。
    #[test]
    fn parallel_plan_without_sources_has_no_online_stage() {
        let plan = ResolutionPlan::new(MediaPlayer::QqMusic, LyricsOnlineStrategy::Parallel, &[]);

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

    /// 当前平台的接口被取消勾选时，仅当前平台策略不发起任何在线检索。
    #[test]
    fn current_player_only_with_unchecked_current_player_has_no_online_stage() {
        let plan = ResolutionPlan::new(
            MediaPlayer::QqMusic,
            LyricsOnlineStrategy::CurrentPlayerOnly,
            &[MediaPlayer::KugouMusic],
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
