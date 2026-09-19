use std::{
    fs::{self, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
    sync::Mutex,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use serde::{Deserialize, Serialize};

use crate::{filesystem, media::MediaPlayer};

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
        filesystem::ensure_managed_directory(app_cache_dir)?;
        let lyrics_path = app_cache_dir.join("lyrics");
        filesystem::ensure_managed_directory(&lyrics_path)?;
        remove_obsolete_schema_directories(&lyrics_path);
        let cache_path = lyrics_path.join(lyrics_cache_schema_label());
        filesystem::ensure_managed_directory(&cache_path)?;
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
        let metadata = filesystem::metadata_without_reparse(&path).ok()?;
        if metadata.len() > MAX_CACHE_ENTRY_BYTES {
            let _ = fs::remove_file(&path);
            self.invalidate_diagnostics();
            return None;
        }
        let content = fs::read(&path).ok()?;
        let entry = serde_json::from_slice::<CacheEntry>(&content).ok();
        if let Some(entry) = entry.filter(|entry| {
            entry.snapshot.track_key.as_deref() == Some(track_key)
                && is_cacheable_status(entry.snapshot.status)
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
        // 瞬时未命中不持久化；真实歌词和已确认纯音乐都可跨播放复用。
        if !is_cacheable_status(snapshot.status) {
            return Ok(());
        }
        self.ensure_directory_boundary()?;
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
        let replaced_bytes = match filesystem::metadata_if_exists(&target)? {
            Some(metadata) if metadata.is_file() => Some(metadata.len()),
            Some(_) => {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    format!("歌词缓存目标不是普通文件: {}", target.display()),
                )
                .into());
            }
            None => None,
        };
        let content_bytes = content.len() as u64;
        let write_result = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary)?
            .write_all(&content);
        if let Err(error) = write_result {
            let _ = fs::remove_file(&temporary);
            return Err(error.into());
        }
        if replaced_bytes.is_some()
            && let Err(error) = fs::remove_file(&target)
        {
            let _ = fs::remove_file(&temporary);
            return Err(error.into());
        }
        match fs::rename(&temporary, &target) {
            Ok(()) => match self.prune_after_write(&target, content_bytes, replaced_bytes) {
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
        self.ensure_directory_boundary()?;
        let path = self.entry_path(track_key);
        let Some(metadata) = filesystem::metadata_if_exists(&path)? else {
            self.record_removed_entry(track_key, None);
            return Ok(());
        };
        if !metadata.is_file() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!("歌词缓存目标不是普通文件: {}", path.display()),
            ));
        }
        let removed_bytes = Some(metadata.len());
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
        self.ensure_directory_boundary()?;
        for entry in fs::read_dir(&self.cache_path)? {
            let entry = entry?;
            if filesystem::entry_metadata_without_reparse(&entry)?.is_file() {
                fs::remove_file(entry.path())?;
            }
        }
        self.invalidate_diagnostics();
        Ok(())
    }

    /// 只删除依赖指定播放器本地目录的结果，在线结果与其他播放器缓存继续保留。
    pub fn clear_local_source(&self, player: MediaPlayer) -> Result<(), std::io::Error> {
        let result = (|| {
            self.ensure_directory_boundary()?;
            for entry in fs::read_dir(&self.cache_path)? {
                let entry = entry?;
                if !filesystem::entry_metadata_without_reparse(&entry)?.is_file() {
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

    /// 写入后的容量维护：已知总量仍在上限内时用增量记账代替整目录扫描。
    /// 诊断状态里的总量只会被高估（所有删除路径都递减或直接失效），因此不会漏判超限。
    fn prune_after_write(
        &self,
        protected_path: &Path,
        written_bytes: u64,
        replaced_bytes: Option<u64>,
    ) -> Result<CacheTotals, std::io::Error> {
        let tracked = self
            .diagnostics
            .lock()
            .ok()
            .and_then(|diagnostics| diagnostics.totals);
        // 首次写入或总量刚被失效时必须真正扫描一次，才能建立可信基线。
        let Some(tracked) = tracked else {
            return self.prune_to_size_limit(protected_path);
        };
        let entry_count = if replaced_bytes.is_some() {
            tracked.entry_count
        } else {
            tracked.entry_count.saturating_add(1)
        };
        let total_bytes = tracked
            .total_bytes
            .saturating_add(written_bytes)
            .saturating_sub(replaced_bytes.unwrap_or(0));
        // 接近或超过上限时只能依赖真实扫描来决定淘汰哪些条目。
        if total_bytes > MAX_CACHE_TOTAL_BYTES {
            return self.prune_to_size_limit(protected_path);
        }
        Ok(CacheTotals {
            entry_count,
            total_bytes,
        })
    }

    /// 按最近写入时间淘汰旧条目，使永久运行也不会无限占用磁盘。
    fn prune_to_size_limit(&self, protected_path: &Path) -> Result<CacheTotals, std::io::Error> {
        self.ensure_directory_boundary()?;
        let mut total_bytes = 0_u64;
        let mut entry_count = 0_usize;
        let mut entries = Vec::new();
        for entry in fs::read_dir(&self.cache_path)? {
            let entry = entry?;
            let metadata = filesystem::entry_metadata_without_reparse(&entry)?;
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

    /// 逐级验证应用缓存根、歌词根和版本目录，避免父级 junction 隐藏最终真实位置。
    fn ensure_directory_boundary(&self) -> Result<(), std::io::Error> {
        let lyrics_path = self.cache_path.parent().ok_or_else(|| {
            std::io::Error::new(std::io::ErrorKind::InvalidData, "歌词缓存目录缺少父级")
        })?;
        let app_cache_dir = lyrics_path.parent().ok_or_else(|| {
            std::io::Error::new(std::io::ErrorKind::InvalidData, "歌词缓存根目录缺少父级")
        })?;
        filesystem::ensure_directory(app_cache_dir)?;
        filesystem::ensure_directory(lyrics_path)?;
        filesystem::ensure_directory(&self.cache_path)
    }
}

/// 将旧的 `vN/entries/*.json` 原地迁移到版本目录，失败时保留旧目录供下次重试。
fn migrate_legacy_entries_directory(cache_path: &Path) {
    let legacy_path = cache_path.join("entries");
    let Ok(legacy_metadata) = filesystem::metadata_without_reparse(&legacy_path) else {
        return;
    };
    if !legacy_metadata.is_dir() {
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
        let Ok(metadata) = filesystem::entry_metadata_without_reparse(&entry) else {
            migration_failed = true;
            continue;
        };
        if !metadata.is_file() {
            continue;
        }
        let source = entry.path();
        if source.extension().and_then(|value| value.to_str()) != Some("json") {
            continue;
        }
        let target = cache_path.join(entry.file_name());
        let result = match filesystem::metadata_without_reparse(&target) {
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                fs::rename(&source, &target)
            }
            Ok(metadata) if metadata.is_file() => fs::remove_file(&source),
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
    if let Err(error) = filesystem::ensure_directory(lyrics_path) {
        log::warn!("歌词缓存根目录边界检查失败: {error}");
        return;
    }
    let Ok(entries) = fs::read_dir(lyrics_path) else {
        return;
    };
    for entry in entries.flatten() {
        let Ok(metadata) = filesystem::entry_metadata_without_reparse(&entry) else {
            continue;
        };
        if !metadata.is_dir() {
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
        LyricsStatus::Ready | LyricsStatus::Instrumental => Some(LINE_REFRESH_INTERVAL),
        LyricsStatus::Loading | LyricsStatus::Unavailable | LyricsStatus::Error => None,
    }
}

/// 只允许可稳定复用的解析结论进入磁盘缓存。
fn is_cacheable_status(status: LyricsStatus) -> bool {
    matches!(status, LyricsStatus::Ready | LyricsStatus::Instrumental)
}

fn now_seconds() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}
