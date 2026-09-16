use crate::media::MediaPlayer;

use super::LyricsOnlineStrategy;

/// 单次歌词来源尝试所调用的能力。
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum ResolutionCapability {
    Local,
    Online,
}

/// 一次可诊断的歌词来源尝试。
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct ResolutionAttempt {
    pub(super) label: &'static str,
    pub(super) player: MediaPlayer,
    pub(super) capability: ResolutionCapability,
}

/// 同一阶段中的尝试；多于一个来源时允许并行执行。
pub(super) struct ResolutionStage {
    pub(super) parallel_group: Option<&'static str>,
    pub(super) attempts: Vec<ResolutionAttempt>,
}

/// 把产品层的来源优先级编译为可执行计划。
pub(super) struct ResolutionPlan {
    pub(super) local_attempts: Vec<ResolutionAttempt>,
    pub(super) online_stages: Vec<ResolutionStage>,
}

impl ResolutionPlan {
    /// 当前播放器先查本地，非 QQ 播放器再使用 QQ 本地缓存兜底。
    pub(super) fn new(current_player: MediaPlayer, strategy: LyricsOnlineStrategy) -> Self {
        let mut local_attempts = vec![ResolutionAttempt {
            label: "当前播放器本地",
            player: current_player,
            capability: ResolutionCapability::Local,
        }];
        if current_player != MediaPlayer::QqMusic {
            local_attempts.push(ResolutionAttempt {
                label: "QQ 本地兜底",
                player: MediaPlayer::QqMusic,
                capability: ResolutionCapability::Local,
            });
        }

        let current_label = match strategy {
            LyricsOnlineStrategy::Parallel => "当前播放器在线",
            LyricsOnlineStrategy::CurrentPlayerFirst => "当前播放器在线优先",
        };
        let current = ResolutionAttempt {
            label: current_label,
            player: current_player,
            capability: ResolutionCapability::Online,
        };
        let mut fallbacks = Vec::with_capacity(2);
        if current_player != MediaPlayer::QqMusic {
            fallbacks.push(ResolutionAttempt {
                label: "QQ 在线兜底",
                player: MediaPlayer::QqMusic,
                capability: ResolutionCapability::Online,
            });
        }
        if current_player != MediaPlayer::NeteaseCloudMusic {
            fallbacks.push(ResolutionAttempt {
                label: "网易云在线兜底",
                player: MediaPlayer::NeteaseCloudMusic,
                capability: ResolutionCapability::Online,
            });
        }

        let online_stages = match strategy {
            LyricsOnlineStrategy::Parallel => {
                let mut attempts = Vec::with_capacity(1 + fallbacks.len());
                attempts.push(current);
                attempts.extend(fallbacks);
                vec![ResolutionStage {
                    parallel_group: (attempts.len() > 1).then_some("并行在线查询"),
                    attempts,
                }]
            }
            LyricsOnlineStrategy::CurrentPlayerFirst => vec![
                ResolutionStage {
                    parallel_group: None,
                    attempts: vec![current],
                },
                ResolutionStage {
                    parallel_group: (fallbacks.len() > 1).then_some("并行在线兜底"),
                    attempts: fallbacks,
                },
            ],
        };

        Self {
            local_attempts,
            online_stages,
        }
    }
}
