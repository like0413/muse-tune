use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};

/// 所有歌词目录 watcher 共享的累计背压指标，跨 watcher 重建保持连续。
#[derive(Default)]
pub(in crate::lyrics) struct LyricsWatcherMetrics {
    enqueued_batches: AtomicU64,
    processed_batches: AtomicU64,
    coalesced_batches: AtomicU64,
    callback_count: AtomicU64,
    pending_batches: AtomicUsize,
    pending_batches_peak: AtomicUsize,
}

impl LyricsWatcherMetrics {
    /// 登记 notify callback 交付的一批有效路径。
    pub(in crate::lyrics) fn record_enqueue(&self) {
        self.enqueued_batches.fetch_add(1, Ordering::Relaxed);
        let pending = self.pending_batches.fetch_add(1, Ordering::Relaxed) + 1;
        self.pending_batches_peak
            .fetch_max(pending, Ordering::Relaxed);
    }

    /// worker 在安静窗口结束后一次消费全部累计批次。
    pub(in crate::lyrics) fn record_processed_batches(&self, batch_count: usize) {
        let processed = u64::try_from(batch_count).unwrap_or(u64::MAX);
        self.processed_batches
            .fetch_add(processed, Ordering::Relaxed);
        self.coalesced_batches
            .fetch_add(processed.saturating_sub(1), Ordering::Relaxed);
        self.pending_batches
            .fetch_sub(batch_count, Ordering::Relaxed);
    }

    /// watcher 重建或退出时移除不再交付的 pending 批次。
    pub(in crate::lyrics) fn discard_pending_batches(&self, batch_count: usize) {
        self.pending_batches
            .fetch_sub(batch_count, Ordering::Relaxed);
    }

    /// 记录最终进入歌词服务的缓存变化回调次数。
    pub(in crate::lyrics) fn record_callback(&self) {
        self.callback_count.fetch_add(1, Ordering::Relaxed);
    }

    /// 返回不包含路径和文件名的轻量诊断快照。
    pub(in crate::lyrics) fn snapshot(&self) -> LyricsWatcherMetricsSnapshot {
        LyricsWatcherMetricsSnapshot {
            enqueued_batches: self.enqueued_batches.load(Ordering::Relaxed),
            processed_batches: self.processed_batches.load(Ordering::Relaxed),
            coalesced_batches: self.coalesced_batches.load(Ordering::Relaxed),
            callback_count: self.callback_count.load(Ordering::Relaxed),
            pending_batches: self.pending_batches.load(Ordering::Relaxed),
            pending_batches_peak: self.pending_batches_peak.load(Ordering::Relaxed),
        }
    }
}

/// 歌词目录 watcher 指标的单次一致字段集合。
pub(in crate::lyrics) struct LyricsWatcherMetricsSnapshot {
    pub enqueued_batches: u64,
    pub processed_batches: u64,
    pub coalesced_batches: u64,
    pub callback_count: u64,
    pub pending_batches: usize,
    pub pending_batches_peak: usize,
}
