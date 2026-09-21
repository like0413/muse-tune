//! 歌词快照的读取与发布。
//!
//! 所有发布都先校验解析代际：解析一轮可能要几百毫秒，期间用户随时可能切歌，
//! 少了这一步，慢请求的结果会覆盖刚切换到的新歌曲。

use std::sync::atomic::Ordering;

use crate::lyrics::model::{LyricsResolutionMethod, LyricsSnapshot, LyricsStatus};

use super::LyricsService;

impl LyricsService {
    pub fn snapshot(&self) -> LyricsSnapshot {
        self.inner.runtime_state.read().map_or_else(
            |_| LyricsSnapshot::default(),
            |state| state.snapshot.clone(),
        )
    }

    /// 发布“本次没有拿到歌词”的收尾结论。
    ///
    /// 只有所有来源都正常执行并返回“没找到”时才说这首歌没有歌词；只要有来源技术性失败，
    /// 就按错误上报——两者在界面上都表现为没有歌词可显示，但前者重试无意义，后者往往只是
    /// 网络或平台暂时不可用，混为一谈会让用户以为这首歌根本没有歌词。
    pub(super) fn publish_no_lyrics(
        &self,
        track_key: &str,
        generation: u64,
        source_failed: bool,
        miss_reason: &str,
        failure_reason: &str,
    ) {
        let snapshot = if source_failed {
            LyricsSnapshot {
                track_key: Some(track_key.to_owned()),
                status: LyricsStatus::Error,
                error_reason: Some(failure_reason.to_owned()),
                ..LyricsSnapshot::default()
            }
        } else {
            LyricsSnapshot::unavailable(Some(track_key.to_owned()), miss_reason)
        };
        self.store_and_publish_if_current(snapshot, generation, LyricsResolutionMethod::None);
    }

    /// 持久化解析结果，并仅在请求仍对应当前歌曲时发布，避免慢请求覆盖新歌曲。
    pub(super) fn store_and_publish_if_current(
        &self,
        snapshot: LyricsSnapshot,
        generation: u64,
        resolution_method: LyricsResolutionMethod,
    ) {
        // 代数检查必须与歌曲身份更新互斥，否则旧任务可能覆盖新歌曲。
        if !self.current_generation_matches(generation) {
            return;
        }
        // 磁盘写入是本流程最慢的一步，移出歌曲身份锁，避免阻塞媒体监控线程更新当前歌曲。
        if let Err(error) = self.inner.cache.store(&snapshot, generation) {
            log::warn!("保存解析后歌词缓存失败: {error}");
        }
        // 写入期间可能已经切歌，发布前重新校验，避免把过期结果广播出去。
        self.publish_if_current_with_method(snapshot, generation, resolution_method);
    }

    /// 在歌曲身份锁内校验代数，供锁外工作的入口与出口复用。
    pub(super) fn current_generation_matches(&self, generation: u64) -> bool {
        let Ok(_current) = self.inner.current_track.lock() else {
            return false;
        };
        self.is_current_generation(generation)
    }

    pub(super) fn publish_if_current(&self, snapshot: LyricsSnapshot, generation: u64) {
        self.publish_if_current_with_method(snapshot, generation, LyricsResolutionMethod::None);
    }

    pub(super) fn publish_if_current_with_method(
        &self,
        snapshot: LyricsSnapshot,
        generation: u64,
        resolution_method: LyricsResolutionMethod,
    ) {
        // 与歌曲身份更新使用同一把锁，使“检查代数 → 发布”不会被切歌事件穿插。
        let Ok(_current) = self.inner.current_track.lock() else {
            return;
        };
        if self.is_current_generation(generation) {
            self.publish(snapshot, resolution_method);
        }
    }

    pub(super) fn is_current_generation(&self, generation: u64) -> bool {
        self.inner.generation.load(Ordering::Acquire) == generation
    }

    fn publish(&self, snapshot: LyricsSnapshot, resolution_method: LyricsResolutionMethod) {
        let snapshot_changed = self.inner.runtime_state.write().map_or(true, |mut state| {
            let changed = state.snapshot != snapshot;
            if changed {
                state.snapshot.clone_from(&snapshot);
            }
            state.resolution_method = resolution_method;
            changed
        });
        // 后台校验可能再次读出同一缓存；重复事件会让 WebView 重算歌词 DOM 并产生闪烁。
        if snapshot_changed {
            (self.inner.publisher)(&snapshot);
        }
        (self.inner.diagnostics_notifier)();
    }
}
