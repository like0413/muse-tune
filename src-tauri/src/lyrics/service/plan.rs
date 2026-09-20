use crate::media::MediaPlayer;

use super::players;
use super::{LyricsOnlineStrategy, LyricsParallelGroup, LyricsResolutionSite};

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
