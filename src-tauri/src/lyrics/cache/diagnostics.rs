//! 歌词缓存的轻量索引与诊断快照。

use std::{fs, path::Path, sync::Mutex, time::Duration};

use crate::lyrics::{model::LyricsCacheDiagnostics, schema::lyrics_cache_schema_label};

use super::{
    MAX_CACHE_TOTAL_BYTES,
    entry::{CacheEntry, decode_entry},
    freshness::{now_seconds, refresh_interval},
};

#[derive(Clone, Copy, Default)]
pub(super) struct CacheTotals {
    pub(super) entry_count: usize,
    pub(super) total_bytes: u64,
}

#[derive(Clone)]
struct CurrentCacheEntry {
    bytes: u64,
    refreshed_at_seconds: u64,
    refresh_interval: Option<Duration>,
}

#[derive(Default)]
struct CacheDiagnosticsState {
    revision: u64,
    totals: Option<CacheTotals>,
    current_track_key: Option<String>,
    current_entry: Option<CurrentCacheEntry>,
}

/// 缓存目录的延迟统计索引；文件发生变化时通过 revision 使旧扫描结果失效。
#[derive(Default)]
pub(super) struct CacheDiagnostics {
    state: Mutex<CacheDiagnosticsState>,
}

impl CacheDiagnostics {
    /// 读取缓存占用和当前歌曲条目状态；文件未变化时复用轻量索引。
    pub(super) fn snapshot(
        &self,
        cache_path: &Path,
        track_key: Option<&str>,
    ) -> LyricsCacheDiagnostics {
        let Ok(state) = self.state.lock() else {
            return LyricsCacheDiagnostics {
                schema_version: lyrics_cache_schema_label(),
                limit_bytes: MAX_CACHE_TOTAL_BYTES,
                ..LyricsCacheDiagnostics::default()
            };
        };
        let revision = state.revision;
        let cached_totals = state.totals;
        let cached_current =
            (state.current_track_key.as_deref() == track_key).then(|| state.current_entry.clone());
        drop(state);

        let totals = cached_totals.unwrap_or_else(|| scan_cache_totals(cache_path));
        let current_was_cached = cached_current.is_some();
        let current_entry = cached_current
            .unwrap_or_else(|| track_key.and_then(|key| read_current_entry(cache_path, key)));
        if (cached_totals.is_none() || !current_was_cached)
            && let Ok(mut state) = self.state.lock()
            && state.revision == revision
        {
            state.totals.get_or_insert(totals);
            if !current_was_cached {
                state.current_track_key = track_key.map(str::to_owned);
                state.current_entry.clone_from(&current_entry);
            }
        }

        let mut result = LyricsCacheDiagnostics {
            schema_version: lyrics_cache_schema_label(),
            entry_count: totals.entry_count,
            total_bytes: totals.total_bytes,
            limit_bytes: MAX_CACHE_TOTAL_BYTES,
            ..LyricsCacheDiagnostics::default()
        };
        let Some(entry) = current_entry.as_ref() else {
            return result;
        };
        result.current_entry_exists = true;
        result.current_entry_bytes = Some(entry.bytes);
        let age = now_seconds().checked_sub(entry.refreshed_at_seconds);
        result.current_entry_age_seconds = age;
        result.current_entry_fresh = Some(
            age.zip(entry.refresh_interval)
                .is_some_and(|(age, interval)| age < interval.as_secs()),
        );
        result.current_refresh_remaining_seconds = age
            .zip(entry.refresh_interval)
            .map(|(age, interval)| interval.as_secs().saturating_sub(age));
        result
    }

    pub(super) fn tracked_totals(&self) -> Option<CacheTotals> {
        self.state.lock().ok().and_then(|state| state.totals)
    }

    pub(super) fn invalidate(&self) {
        if let Ok(mut state) = self.state.lock() {
            state.revision = state.revision.wrapping_add(1);
            state.totals = None;
            state.current_track_key = None;
            state.current_entry = None;
        }
    }

    pub(super) fn record_current(&self, track_key: &str, bytes: u64, entry: &CacheEntry) {
        if let Ok(mut state) = self.state.lock() {
            state.current_track_key = Some(track_key.to_owned());
            state.current_entry = Some(current_cache_entry(bytes, entry));
        }
    }

    pub(super) fn record_stored(
        &self,
        track_key: &str,
        bytes: u64,
        entry: &CacheEntry,
        totals: CacheTotals,
    ) {
        if let Ok(mut state) = self.state.lock() {
            state.revision = state.revision.wrapping_add(1);
            state.totals = Some(totals);
            state.current_track_key = Some(track_key.to_owned());
            state.current_entry = Some(current_cache_entry(bytes, entry));
        }
    }

    pub(super) fn record_removed(&self, track_key: &str, removed_bytes: Option<u64>) {
        if let Ok(mut state) = self.state.lock() {
            state.revision = state.revision.wrapping_add(1);
            if let (Some(totals), Some(bytes)) = (state.totals.as_mut(), removed_bytes) {
                totals.entry_count = totals.entry_count.saturating_sub(1);
                totals.total_bytes = totals.total_bytes.saturating_sub(bytes);
            } else if removed_bytes.is_some() {
                state.totals = None;
            }
            if state.current_track_key.as_deref() == Some(track_key) {
                state.current_entry = None;
            }
        }
    }
}

fn scan_cache_totals(cache_path: &Path) -> CacheTotals {
    let mut totals = CacheTotals::default();
    if let Ok(entries) = fs::read_dir(cache_path) {
        for metadata in entries
            .flatten()
            .filter_map(|entry| entry.metadata().ok())
            .filter(|metadata| metadata.is_file())
        {
            totals.entry_count += 1;
            totals.total_bytes = totals.total_bytes.saturating_add(metadata.len());
        }
    }
    totals
}

fn read_current_entry(cache_path: &Path, track_key: &str) -> Option<CurrentCacheEntry> {
    let path = cache_path.join(format!("{track_key}.bin"));
    let metadata = fs::metadata(&path).ok()?;
    let entry = fs::read(path)
        .ok()
        .and_then(|content| decode_entry(&content))?;
    Some(current_cache_entry(metadata.len(), &entry))
}

fn current_cache_entry(bytes: u64, entry: &CacheEntry) -> CurrentCacheEntry {
    CurrentCacheEntry {
        bytes,
        refreshed_at_seconds: entry.refreshed_at_seconds,
        refresh_interval: refresh_interval(entry),
    }
}
