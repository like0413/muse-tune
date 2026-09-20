//! 设置页与数据页需要的只读诊断视图。
//!
//! 这里只做读取与汇总，不改变任何运行状态：诊断入口随时可能被界面调用，
//! 一旦带副作用就会干扰正在进行的解析。

use std::collections::HashSet;

use crate::lyrics::{
    model::{
        LyricsAdapterDiagnostics, LyricsCacheDiagnostics, LyricsDiagnostics,
        LyricsResolutionMethod, LyricsSnapshotDiagnostics, LyricsWatcherDiagnostics,
    },
    players,
};

use super::LyricsService;

impl LyricsService {
    /// 返回当前来源、自动目录与调度器状态，供设置页只读诊断。
    pub fn diagnostics(&self) -> LyricsDiagnostics {
        let current_track = self
            .inner
            .current_track
            .lock()
            .ok()
            .and_then(|track| track.clone());
        let current_player = current_track.as_ref().map(|track| track.player);
        let (resolver_running, pending_resolution) = self
            .inner
            .resolver
            .lock()
            .map_or((false, false), |resolver| {
                (resolver.running, resolver.pending.is_some())
            });
        let (
            snapshot,
            resolution_method,
            resolution_duration_ms,
            resolution_track,
            resolution_steps,
            recent_resolutions,
        ) = self.inner.runtime_state.read().map_or_else(
            |_| {
                (
                    LyricsSnapshotDiagnostics::default(),
                    LyricsResolutionMethod::None,
                    None,
                    None,
                    Vec::new(),
                    Vec::new(),
                )
            },
            |state| {
                (
                    LyricsSnapshotDiagnostics {
                        status: state.snapshot.status,
                        source: state.snapshot.source.clone(),
                        precision: state.snapshot.precision,
                        line_count: state.snapshot.lines.len(),
                        error_reason: state.snapshot.error_reason.clone(),
                    },
                    state.resolution_method,
                    state.resolution_duration_ms,
                    state.resolution_track.clone(),
                    state.resolution_steps.clone(),
                    state.recent_resolutions.clone(),
                )
            },
        );
        let active_watchers = self
            .inner
            .watchers
            .lock()
            .map(|watchers| watchers.keys().copied().collect::<HashSet<_>>())
            .unwrap_or_default();
        let discovered_paths = self.inner.adapter_paths.read().map_or_else(
            |_| Vec::new(),
            |paths| {
                players::supported_players()
                    .map(|player| {
                        let cache_path = paths.get(&player).cloned().flatten();
                        let available = cache_path.as_ref().is_some_and(|path| path.is_dir());
                        (player, cache_path, available)
                    })
                    .collect::<Vec<_>>()
            },
        );
        let (local_cache_path, local_cache_available) = current_player
            .and_then(|current_player| {
                discovered_paths
                    .iter()
                    .find(|(player, _, _)| *player == current_player)
                    .map(|(_, path, available)| (path.clone(), *available))
            })
            .unwrap_or_default();
        let adapters = discovered_paths
            .into_iter()
            .map(
                |(player, cache_path, cache_path_available)| LyricsAdapterDiagnostics {
                    player,
                    cache_path_available,
                    cache_path: cache_path.map(|path| display_path(&path)),
                    watcher_active: active_watchers.contains(&player),
                },
            )
            .collect();
        let preferences = self.preferences();
        LyricsDiagnostics {
            snapshot,
            current_player,
            enabled: preferences.enabled,
            online_strategy: preferences.online_strategy,
            resolution_method,
            local_cache_available,
            local_cache_path: local_cache_path.map(|path| display_path(&path)),
            resolver_running,
            pending_resolution,
            resolution_duration_ms,
            resolution_track,
            resolution_steps,
            recent_resolutions,
            cache: self
                .inner
                .cache
                .diagnostics(current_track.as_ref().map(|track| track.key.as_str())),
            adapters,
            watcher: {
                let metrics = self.inner.watcher_metrics.snapshot();
                LyricsWatcherDiagnostics {
                    enqueued_batches: metrics.enqueued_batches,
                    processed_batches: metrics.processed_batches,
                    coalesced_batches: metrics.coalesced_batches,
                    callback_count: metrics.callback_count,
                    pending_batches: metrics.pending_batches,
                    pending_batches_peak: metrics.pending_batches_peak,
                }
            },
        }
    }

    /// 返回缓存容量与占用摘要，供数据页独立读取。
    pub fn cache_diagnostics(&self) -> LyricsCacheDiagnostics {
        let current_track = self
            .inner
            .current_track
            .lock()
            .ok()
            .and_then(|track| track.clone());
        self.inner
            .cache
            .diagnostics(current_track.as_ref().map(|track| track.key.as_str()))
    }

    /// 返回歌词缓存根目录，例如 `lyrics`。
    pub fn cache_directory(&self) -> &std::path::Path {
        self.inner.cache.directory()
    }
}

/// 去掉 Windows 扩展长度前缀，让诊断里显示的路径与用户看到的一致。
fn display_path(path: &std::path::Path) -> String {
    let value = path.to_string_lossy();
    value.strip_prefix(r"\\?\").unwrap_or(&value).to_owned()
}
