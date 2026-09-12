use std::{
    fs,
    path::{Path, PathBuf},
    sync::Mutex,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use serde::{Deserialize, Serialize};

use crate::media::MediaPlayer;

use super::{
    error::LyricsError,
    model::{
        LyricsCacheDiagnostics, LyricsPrecision, LyricsSnapshot, LyricsSourceKind, LyricsStatus,
    },
    schema::{LYRICS_CACHE_SCHEMA_VERSION, lyrics_cache_schema_label},
};

const MAX_CACHE_ENTRY_BYTES: u64 = 2 * 1024 * 1024;
const MAX_CACHE_TOTAL_BYTES: u64 = 256 * 1024 * 1024;
const WORD_REFRESH_INTERVAL: Duration = Duration::from_secs(30 * 24 * 60 * 60);
const LINE_REFRESH_INTERVAL: Duration = Duration::from_secs(7 * 24 * 60 * 60);
const UNAVAILABLE_REFRESH_INTERVAL: Duration = Duration::from_secs(6 * 60 * 60);

/// 缓存命中及其是否仍处于免联网刷新期。
pub struct CacheLookup {
    pub snapshot: LyricsSnapshot,
    pub is_fresh: bool,
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct CacheEntry {
    refreshed_at_seconds: u64,
    snapshot: LyricsSnapshot,
}

/// 版本化的解析后歌词文件缓存。
pub struct ParsedLyricsCache {
    cache_path: PathBuf,
    diagnostics: Mutex<CacheDiagnosticsState>,
}

#[derive(Clone, Copy, Default)]
struct CacheTotals {
    entry_count: usize,
    total_bytes: u64,
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

impl ParsedLyricsCache {
    /// 在应用缓存目录下创建歌词专用版本目录。
    pub fn new(app_cache_dir: &Path) -> Result<Self, std::io::Error> {
        let lyrics_path = app_cache_dir.join("lyrics");
        fs::create_dir_all(&lyrics_path)?;
        remove_obsolete_schema_directories(&lyrics_path);
        let cache_path = lyrics_path.join(lyrics_cache_schema_label());
        fs::create_dir_all(&cache_path)?;
        migrate_legacy_entries_directory(&cache_path);
        Ok(Self {
            cache_path,
            diagnostics: Mutex::new(CacheDiagnosticsState::default()),
        })
    }

    /// 返回歌词缓存根目录，供数据页查看全部版本目录。
    pub fn directory(&self) -> &Path {
        self.cache_path.parent().unwrap_or(&self.cache_path)
    }

    /// 读取并校验单个缓存条目，损坏条目按未命中处理。
    pub fn load(&self, track_key: &str) -> Option<CacheLookup> {
        let path = self.entry_path(track_key);
        let metadata = fs::metadata(&path).ok()?;
        if metadata.len() > MAX_CACHE_ENTRY_BYTES {
            let _ = fs::remove_file(&path);
            self.invalidate_diagnostics();
            return None;
        }
        let content = fs::read(&path).ok()?;
        let entry = serde_json::from_slice::<CacheEntry>(&content).ok();
        if let Some(entry) = entry.filter(|entry| {
            entry.snapshot.track_key.as_deref() == Some(track_key)
                && matches!(
                    entry.snapshot.status,
                    LyricsStatus::Ready | LyricsStatus::Unavailable
                )
        }) {
            let is_fresh = is_fresh(&entry);
            self.record_current_entry(track_key, metadata.len(), &entry);
            return Some(CacheLookup {
                is_fresh,
                snapshot: entry.snapshot,
            });
        }
        let _ = fs::remove_file(path);
        self.invalidate_diagnostics();
        None
    }

    /// 写入本次解析结果和刷新时间；损坏缓存可在下次播放时自动重建。
    pub fn store(&self, snapshot: &LyricsSnapshot) -> Result<(), LyricsError> {
        let Some(track_key) = snapshot.track_key.as_deref() else {
            return Ok(());
        };
        if !matches!(
            snapshot.status,
            LyricsStatus::Ready | LyricsStatus::Unavailable
        ) {
            return Ok(());
        }
        let target = self.entry_path(track_key);
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        let temporary =
            self.cache_path
                .join(format!(".{track_key}.{}.{}.tmp", std::process::id(), nonce));
        let cache_entry = CacheEntry {
            refreshed_at_seconds: now_seconds(),
            snapshot: snapshot.clone(),
        };
        let content = serde_json::to_vec(&cache_entry)?;
        if content.len() as u64 > MAX_CACHE_ENTRY_BYTES {
            return Err(LyricsError::InvalidData(
                "规范化歌词超过缓存大小上限".to_owned(),
            ));
        }
        let content_bytes = content.len() as u64;
        fs::write(&temporary, content)?;
        if target.exists()
            && let Err(error) = fs::remove_file(&target)
        {
            let _ = fs::remove_file(temporary);
            return Err(error.into());
        }
        match fs::rename(&temporary, &target) {
            Ok(()) => match self.prune_to_size_limit(&target) {
                Ok(totals) => {
                    self.record_stored_entry(track_key, content_bytes, &cache_entry, totals);
                    Ok(())
                }
                Err(error) => {
                    self.invalidate_diagnostics();
                    Err(error.into())
                }
            },
            Err(error) => {
                let _ = fs::remove_file(temporary);
                self.invalidate_diagnostics();
                Err(error.into())
            }
        }
    }

    /// 删除当前歌曲的解析结果，使播放器源文件变化后只重建受影响条目。
    pub fn remove(&self, track_key: &str) -> Result<(), std::io::Error> {
        let path = self.entry_path(track_key);
        let removed_bytes = fs::metadata(&path).ok().map(|metadata| metadata.len());
        match fs::remove_file(path) {
            Ok(()) => {
                self.record_removed_entry(track_key, removed_bytes);
                Ok(())
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                self.record_removed_entry(track_key, None);
                Ok(())
            }
            Err(error) => Err(error),
        }
    }

    /// 清空全部规范化歌词缓存，同时保留版本目录供后续写入复用。
    pub fn clear(&self) -> Result<(), std::io::Error> {
        for entry in fs::read_dir(&self.cache_path)? {
            let entry = entry?;
            if entry.file_type()?.is_file() {
                fs::remove_file(entry.path())?;
            }
        }
        self.invalidate_diagnostics();
        Ok(())
    }

    /// 只删除依赖指定播放器本地目录的结果，在线结果与其他播放器缓存继续保留。
    pub fn clear_local_source(&self, player: MediaPlayer) -> Result<(), std::io::Error> {
        let result = (|| {
            for entry in fs::read_dir(&self.cache_path)? {
                let entry = entry?;
                if !entry.file_type()?.is_file() {
                    continue;
                }
                let path = entry.path();
                let should_remove = fs::read(&path)
                    .ok()
                    .and_then(|content| serde_json::from_slice::<CacheEntry>(&content).ok())
                    .is_some_and(|entry| {
                        entry.snapshot.source.is_some_and(|source| {
                            source.player == player && source.kind == LyricsSourceKind::Local
                        })
                    });
                if should_remove {
                    fs::remove_file(path)?;
                }
            }
            Ok(())
        })();
        self.invalidate_diagnostics();
        result
    }

    /// 读取缓存占用和当前歌曲条目状态；文件未变化时复用轻量索引。
    pub fn diagnostics(&self, track_key: Option<&str>) -> LyricsCacheDiagnostics {
        let Ok(state) = self.diagnostics.lock() else {
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

        let totals = cached_totals.unwrap_or_else(|| self.scan_cache_totals());
        let current_was_cached = cached_current.is_some();
        let current_entry = cached_current
            .unwrap_or_else(|| track_key.and_then(|key| self.read_current_entry(key)));
        if (cached_totals.is_none() || !current_was_cached)
            && let Ok(mut state) = self.diagnostics.lock()
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
        let current = now_seconds();
        let age = current.checked_sub(entry.refreshed_at_seconds);
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

    fn scan_cache_totals(&self) -> CacheTotals {
        let mut totals = CacheTotals::default();
        if let Ok(entries) = fs::read_dir(&self.cache_path) {
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

    fn read_current_entry(&self, track_key: &str) -> Option<CurrentCacheEntry> {
        let path = self.entry_path(track_key);
        let metadata = fs::metadata(&path).ok()?;
        let entry = fs::read(path)
            .ok()
            .and_then(|content| serde_json::from_slice::<CacheEntry>(&content).ok())?;
        Some(CurrentCacheEntry {
            bytes: metadata.len(),
            refreshed_at_seconds: entry.refreshed_at_seconds,
            refresh_interval: refresh_interval(&entry),
        })
    }

    fn invalidate_diagnostics(&self) {
        if let Ok(mut diagnostics) = self.diagnostics.lock() {
            diagnostics.revision = diagnostics.revision.wrapping_add(1);
            diagnostics.totals = None;
            diagnostics.current_track_key = None;
            diagnostics.current_entry = None;
        }
    }

    fn record_current_entry(&self, track_key: &str, bytes: u64, entry: &CacheEntry) {
        if let Ok(mut diagnostics) = self.diagnostics.lock() {
            diagnostics.current_track_key = Some(track_key.to_owned());
            diagnostics.current_entry = Some(CurrentCacheEntry {
                bytes,
                refreshed_at_seconds: entry.refreshed_at_seconds,
                refresh_interval: refresh_interval(entry),
            });
        }
    }

    fn record_stored_entry(
        &self,
        track_key: &str,
        bytes: u64,
        entry: &CacheEntry,
        totals: CacheTotals,
    ) {
        if let Ok(mut diagnostics) = self.diagnostics.lock() {
            diagnostics.revision = diagnostics.revision.wrapping_add(1);
            diagnostics.totals = Some(totals);
            diagnostics.current_track_key = Some(track_key.to_owned());
            diagnostics.current_entry = Some(CurrentCacheEntry {
                bytes,
                refreshed_at_seconds: entry.refreshed_at_seconds,
                refresh_interval: refresh_interval(entry),
            });
        }
    }

    fn record_removed_entry(&self, track_key: &str, removed_bytes: Option<u64>) {
        if let Ok(mut diagnostics) = self.diagnostics.lock() {
            diagnostics.revision = diagnostics.revision.wrapping_add(1);
            if let (Some(totals), Some(bytes)) = (diagnostics.totals.as_mut(), removed_bytes) {
                totals.entry_count = totals.entry_count.saturating_sub(1);
                totals.total_bytes = totals.total_bytes.saturating_sub(bytes);
            } else if removed_bytes.is_some() {
                diagnostics.totals = None;
            }
            if diagnostics.current_track_key.as_deref() == Some(track_key) {
                diagnostics.current_entry = None;
            }
        }
    }

    fn entry_path(&self, track_key: &str) -> PathBuf {
        self.cache_path.join(format!("{track_key}.json"))
    }

    /// 按最近写入时间淘汰旧条目，使永久运行也不会无限占用磁盘。
    fn prune_to_size_limit(&self, protected_path: &Path) -> Result<CacheTotals, std::io::Error> {
        let mut total_bytes = 0_u64;
        let mut entry_count = 0_usize;
        let mut entries = Vec::new();
        for entry in fs::read_dir(&self.cache_path)? {
            let entry = entry?;
            let metadata = entry.metadata()?;
            if !metadata.is_file() {
                continue;
            }
            entry_count += 1;
            total_bytes = total_bytes.saturating_add(metadata.len());
            entries.push((
                metadata.modified().unwrap_or(UNIX_EPOCH),
                metadata.len(),
                entry.path(),
            ));
        }
        if total_bytes <= MAX_CACHE_TOTAL_BYTES {
            return Ok(CacheTotals {
                entry_count,
                total_bytes,
            });
        }
        entries.sort_unstable_by_key(|(modified, _, _)| *modified);
        for (_, size, path) in entries {
            if total_bytes <= MAX_CACHE_TOTAL_BYTES {
                break;
            }
            if path == protected_path {
                continue;
            }
            fs::remove_file(path)?;
            entry_count = entry_count.saturating_sub(1);
            total_bytes = total_bytes.saturating_sub(size);
        }
        Ok(CacheTotals {
            entry_count,
            total_bytes,
        })
    }
}

/// 将旧的 `vN/entries/*.json` 原地迁移到版本目录，失败时保留旧目录供下次重试。
fn migrate_legacy_entries_directory(cache_path: &Path) {
    let legacy_path = cache_path.join("entries");
    let Ok(legacy_metadata) = fs::symlink_metadata(&legacy_path) else {
        return;
    };
    if !legacy_metadata.is_dir() || legacy_metadata.file_type().is_symlink() {
        return;
    }
    let Ok(entries) = fs::read_dir(&legacy_path) else {
        return;
    };
    let mut migration_failed = false;
    for entry in entries {
        let Ok(entry) = entry else {
            migration_failed = true;
            continue;
        };
        let Ok(file_type) = entry.file_type() else {
            migration_failed = true;
            continue;
        };
        if !file_type.is_file() || file_type.is_symlink() {
            continue;
        }
        let source = entry.path();
        if source.extension().and_then(|value| value.to_str()) != Some("json") {
            continue;
        }
        let target = cache_path.join(entry.file_name());
        let result = match fs::symlink_metadata(&target) {
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                fs::rename(&source, &target)
            }
            Ok(metadata) if metadata.is_file() && !metadata.file_type().is_symlink() => {
                fs::remove_file(&source)
            }
            Ok(_) => {
                log::warn!("迁移歌词缓存时目标路径不是普通文件: {}", target.display());
                migration_failed = true;
                continue;
            }
            Err(error) => Err(error),
        };
        if let Err(error) = result {
            log::warn!("迁移旧版歌词缓存 {} 失败: {error}", source.display());
            migration_failed = true;
        }
    }
    if !migration_failed && let Err(error) = fs::remove_dir_all(&legacy_path) {
        log::warn!("删除已迁移的歌词 entries 目录失败: {error}");
    }
}

/// 启动时仅清理旧版目录；未知目录和更高版本需保留，以支持安全回退。
fn remove_obsolete_schema_directories(lyrics_path: &Path) {
    let Ok(entries) = fs::read_dir(lyrics_path) else {
        return;
    };
    for entry in entries.flatten() {
        let Ok(file_type) = entry.file_type() else {
            continue;
        };
        if !file_type.is_dir() || file_type.is_symlink() {
            continue;
        }
        let Some(version) = entry
            .file_name()
            .to_str()
            .and_then(|name| name.strip_prefix('v'))
            .and_then(|version| version.parse::<u32>().ok())
        else {
            continue;
        };
        if version >= LYRICS_CACHE_SCHEMA_VERSION {
            continue;
        }
        if let Err(error) = fs::remove_dir_all(entry.path()) {
            log::warn!("清理旧版歌词缓存 v{version} 失败: {error}");
        }
    }
}

fn is_fresh(entry: &CacheEntry) -> bool {
    is_fresh_at(entry, now_seconds())
}

fn is_fresh_at(entry: &CacheEntry, current_seconds: u64) -> bool {
    let Some(age) = current_seconds.checked_sub(entry.refreshed_at_seconds) else {
        return false;
    };
    let Some(interval) = refresh_interval(entry) else {
        return false;
    };
    age < interval.as_secs()
}

fn refresh_interval(entry: &CacheEntry) -> Option<Duration> {
    match entry.snapshot.status {
        LyricsStatus::Ready if entry.snapshot.precision == Some(LyricsPrecision::Word) => {
            Some(WORD_REFRESH_INTERVAL)
        }
        LyricsStatus::Ready => Some(LINE_REFRESH_INTERVAL),
        LyricsStatus::Unavailable => Some(UNAVAILABLE_REFRESH_INTERVAL),
        LyricsStatus::Loading | LyricsStatus::Error => None,
    }
}

fn now_seconds() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

#[cfg(test)]
mod tests {
    use super::{
        CacheEntry, LINE_REFRESH_INTERVAL, LyricsPrecision, LyricsSnapshot, LyricsStatus,
        WORD_REFRESH_INTERVAL, is_fresh_at,
    };

    fn entry(status: LyricsStatus, precision: Option<LyricsPrecision>) -> CacheEntry {
        CacheEntry {
            refreshed_at_seconds: 100,
            snapshot: LyricsSnapshot {
                status,
                precision,
                ..LyricsSnapshot::default()
            },
        }
    }

    #[test]
    fn word_cache_expires_after_long_refresh_interval() {
        let entry = entry(LyricsStatus::Ready, Some(LyricsPrecision::Word));

        assert!(!is_fresh_at(&entry, 100 + WORD_REFRESH_INTERVAL.as_secs()));
    }

    #[test]
    fn line_cache_expires_before_word_cache() {
        let entry = entry(LyricsStatus::Ready, Some(LyricsPrecision::Line));

        assert!(!is_fresh_at(&entry, 100 + LINE_REFRESH_INTERVAL.as_secs()));
    }

    #[test]
    fn backward_clock_change_forces_safe_refresh() {
        let entry = entry(LyricsStatus::Ready, Some(LyricsPrecision::Line));

        assert!(!is_fresh_at(&entry, 50));
    }
}
