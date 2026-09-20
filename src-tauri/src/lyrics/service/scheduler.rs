//! 解析任务的排队、取消与串行消费。
//!
//! 同一时刻最多只跑一个主解析线程，期间到达的新请求合并为“最新一条待办”：
//! 歌词解析包含网络往返，并发跑多轮既浪费请求，也会让旧结果有更多机会回写。

use std::{
    panic::{AssertUnwindSafe, catch_unwind},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    thread,
};

use crate::lyrics::{
    cache::CacheLookup,
    model::{LyricsSnapshot, LyricsStatus},
    track::TrackDescriptor,
};

use super::{LyricsService, ResolutionRequest};

impl LyricsService {
    /// 合并解析期间到达的新请求，始终只保留最新歌曲且最多运行一个主解析线程。
    pub(super) fn enqueue_resolution(
        &self,
        track: TrackDescriptor,
        generation: u64,
        cached: Option<CacheLookup>,
        cache_cleared: bool,
    ) {
        let track_key = track.key.clone();
        let cancellation = Arc::new(AtomicBool::new(false));
        let should_start = {
            let Ok(mut resolver) = self.inner.resolver.lock() else {
                return;
            };
            if let Some(active) = resolver.active_cancellation.as_ref() {
                active.store(true, Ordering::Release);
            }
            if let Some(pending) = resolver.pending.as_ref() {
                pending.cancellation.store(true, Ordering::Release);
            }
            resolver.pending = Some(ResolutionRequest {
                track,
                generation,
                cancellation,
                cached,
                cache_cleared,
            });
            if resolver.running {
                false
            } else {
                resolver.running = true;
                true
            }
        };
        if !should_start {
            (self.inner.diagnostics_notifier)();
            return;
        }
        (self.inner.diagnostics_notifier)();
        let service = self.clone();
        if let Err(error) = thread::Builder::new()
            .name("lyrics-resolver".to_owned())
            .spawn(move || service.run_resolution_queue())
        {
            if let Ok(mut resolver) = self.inner.resolver.lock() {
                resolver.running = false;
                resolver.pending = None;
            }
            self.publish_if_current(
                LyricsSnapshot {
                    track_key: Some(track_key),
                    status: LyricsStatus::Error,
                    error_reason: Some(format!("启动歌词解析线程失败: {error}")),
                    ..LyricsSnapshot::default()
                },
                generation,
            );
            (self.inner.diagnostics_notifier)();
        }
    }

    /// 在没有替代任务时取消活动请求并丢弃尚未开始的旧请求。
    pub(super) fn cancel_resolution(&self) {
        let Ok(mut resolver) = self.inner.resolver.lock() else {
            return;
        };
        if let Some(active) = resolver.active_cancellation.as_ref() {
            active.store(true, Ordering::Release);
        }
        if let Some(pending) = resolver.pending.take() {
            pending.cancellation.store(true, Ordering::Release);
        }
        drop(resolver);
        (self.inner.diagnostics_notifier)();
    }

    /// 串行消费最新请求；旧请求结束后跳过所有已经被更新请求覆盖的中间状态。
    fn run_resolution_queue(&self) {
        loop {
            let request = {
                let Ok(mut resolver) = self.inner.resolver.lock() else {
                    return;
                };
                let Some(request) = resolver.pending.take() else {
                    resolver.running = false;
                    resolver.active_cancellation = None;
                    drop(resolver);
                    (self.inner.diagnostics_notifier)();
                    return;
                };
                resolver.active_cancellation = Some(request.cancellation.clone());
                request
            };
            self.resolve_track_guarded(
                request.track,
                request.generation,
                request.cancellation,
                request.cached,
                request.cache_cleared,
            );
        }
    }

    /// 隔离单个平台或第三方库的非预期 panic，确保状态不会永久停留在加载中。
    fn resolve_track_guarded(
        &self,
        track: TrackDescriptor,
        generation: u64,
        cancellation: Arc<AtomicBool>,
        cached: Option<CacheLookup>,
        cache_cleared: bool,
    ) {
        let track_key = track.key.clone();
        self.begin_resolution_trace(generation, &track);
        let panicked = catch_unwind(AssertUnwindSafe(|| {
            self.resolve_track(track, generation, cancellation, cached, cache_cleared)
        }))
        .is_err();
        self.finish_resolution_trace(generation);
        if panicked {
            self.publish_if_current(
                LyricsSnapshot {
                    track_key: Some(track_key),
                    status: LyricsStatus::Error,
                    error_reason: Some("歌词解析器发生未预期异常".to_owned()),
                    ..LyricsSnapshot::default()
                },
                generation,
            );
        }
    }
}
