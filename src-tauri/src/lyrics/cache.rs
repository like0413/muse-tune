use std::{
    collections::HashMap,
    fs::{self, OpenOptions},
    io::{Read, Write},
    path::{Path, PathBuf},
    sync::Mutex,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use flate2::{Compression, read::DeflateDecoder, write::DeflateEncoder};
use serde::{Deserialize, Serialize};

use crate::{filesystem, media::MediaPlayer};

use super::{
    error::LyricsError,
    model::{LyricsCacheDiagnostics, LyricsChineseVariant, LyricsSnapshot, LyricsSourceKind},
    schema::lyrics_cache_schema_label,
};

mod freshness;
mod migration;

use freshness::{is_cacheable_status, is_fresh, needs_revalidation, now_seconds, refresh_interval};
use migration::{migrate_legacy_entries_directory, remove_obsolete_schema_directories};

/// 解压后的单条缓存上限；压缩只是落盘手段，容量约束始终按规范化歌词本体计算。
const MAX_CACHE_ENTRY_BYTES: u64 = 2 * 1024 * 1024;
const MAX_CACHE_TOTAL_BYTES: u64 = 256 * 1024 * 1024;

/// 缓存命中及其时效状态。
pub struct CacheLookup {
    pub snapshot: LyricsSnapshot,
    /// 条目落盘时已经应用的中文字形目标。
    pub chinese_variant: LyricsChineseVariant,
    /// 保留原来源刷新时间；仅改写字形时不能借机延长缓存有效期。
    pub refreshed_at_seconds: u64,
    /// 本轮读取后已按设置转换并覆盖缓存时，记录转换前字形供解析链路展示。
    pub variant_updated_from: Option<LyricsChineseVariant>,
    /// 缓存归一化时正文是否真的发生字形变化。
    pub variant_text_converted: bool,
    /// 本轮字形转换是否成功持久化；失败时仍可展示内存中的转换结果。
    pub variant_rewrite_succeeded: bool,
    /// “原文”不能从已转换文本无损还原，此时缓存只用于诊断，不能用于展示或候选比较。
    pub requires_original_refresh: bool,
    /// 仍在展示可信期内：可以直接展示，不必阻塞式重解析。
    pub is_fresh: bool,
    /// 命中的是逐行结果：本轮需要静默确认能否升级到逐字（升级成功后缓存变成长期结论）。
    pub needs_revalidation: bool,
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct CacheEntry {
    refreshed_at_seconds: u64,
    chinese_variant: LyricsChineseVariant,
    snapshot: LyricsSnapshot,
}

/// 把条目编码为落盘字节：deflate 压缩的 JSON。
///
/// 逐字歌词把每个字写成一个对象，明文里绝大部分字节都是重复的键名和 `null` 占位，
/// 因此压缩比极高而解压开销相对读盘可忽略。只影响磁盘格式，内存与 IPC 结构不变。
fn encode_entry(entry: &CacheEntry) -> Result<Vec<u8>, LyricsError> {
    let json = serde_json::to_vec(entry)?;
    // 上限约束的是规范化歌词本体，压缩后的字节数比它小得多，不能拿它当判据。
    if json.len() as u64 > MAX_CACHE_ENTRY_BYTES {
        return Err(LyricsError::InvalidData(
            "规范化歌词超过缓存大小上限".to_owned(),
        ));
    }
    let mut encoder = DeflateEncoder::new(Vec::new(), Compression::default());
    encoder.write_all(&json)?;
    Ok(encoder.finish()?)
}

/// 解压单个条目；内容损坏或解压后超过上限时返回 `None`，由调用方按未命中处理。
fn decode_entry(content: &[u8]) -> Option<CacheEntry> {
    let mut json = Vec::new();
    // 压缩流可以声称解压出任意大小：先按上限截断，避免单个损坏文件把内存吃满。
    DeflateDecoder::new(content)
        .take(MAX_CACHE_ENTRY_BYTES + 1)
        .read_to_end(&mut json)
        .ok()?;
    if json.len() as u64 > MAX_CACHE_ENTRY_BYTES {
        return None;
    }
    serde_json::from_slice(&json).ok()
}

/// 把临时文件提交为目标文件，覆盖已有同名文件。
///
/// Windows 上 `fs::rename` 不能覆盖已存在的文件，原先的“先删除再改名”在两步之间被中断
/// （进程被杀、断电）会让缓存条目直接消失——而此刻磁盘上的旧内容其实是完整的，崩溃发生在
/// 提交之前，应该保留旧版本。`MoveFileExW` 把替换做成单次调用，不存在这个中间态。
fn replace_file(source: &Path, target: &Path) -> std::io::Result<()> {
    #[cfg(windows)]
    {
        use windows::Win32::Storage::FileSystem::{
            MOVEFILE_REPLACE_EXISTING, MOVEFILE_WRITE_THROUGH, MoveFileExW,
        };
        use windows::core::PCWSTR;

        let source = wide_null(source);
        let target = wide_null(target);
        // SAFETY: 两个指针都指向以 NUL 结尾的有效 UTF-16 缓冲区，生命周期覆盖整个调用。
        unsafe {
            MoveFileExW(
                PCWSTR(source.as_ptr()),
                PCWSTR(target.as_ptr()),
                MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH,
            )
            .map_err(|error| std::io::Error::other(error.to_string()))
        }
    }
    #[cfg(not(windows))]
    {
        fs::rename(source, target)
    }
}

/// 把路径编码为以 NUL 结尾的 UTF-16，供 Win32 宽字符接口使用。
#[cfg(windows)]
fn wide_null(path: &Path) -> Vec<u16> {
    use std::os::windows::ffi::OsStrExt;

    path.as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect()
}

/// 版本化的解析后歌词文件缓存。
pub struct ParsedLyricsCache {
    cache_path: PathBuf,
    diagnostics: Mutex<CacheDiagnosticsState>,
    /// 每个键最近一次写入所属的解析代数，用于拒绝被取代的旧写入。
    ///
    /// 只增不删：任何路径都不会移除或清空条目（`clear` 也只删磁盘文件），因此它随本次运行
    /// 播放过的曲目键单调增长。单条记录很小（键 + u64），但长时间运行且频繁切歌时内存不会回落，
    /// 这也是它刻意与缓存淘汰逻辑解耦的代价。
    write_generations: Mutex<HashMap<String, u64>>,
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
            write_generations: Mutex::new(HashMap::new()),
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
        let entry = decode_entry(&content);
        if let Some(entry) = entry.filter(|entry| {
            entry.snapshot.track_key.as_deref() == Some(track_key)
                && is_cacheable_status(entry.snapshot.status)
        }) {
            let is_fresh = is_fresh(&entry);
            let revalidate = is_fresh && needs_revalidation(&entry);
            self.record_current_entry(track_key, metadata.len(), &entry);
            return Some(CacheLookup {
                is_fresh,
                needs_revalidation: revalidate,
                chinese_variant: entry.chinese_variant,
                refreshed_at_seconds: entry.refreshed_at_seconds,
                variant_updated_from: None,
                variant_text_converted: false,
                variant_rewrite_succeeded: false,
                requires_original_refresh: false,
                snapshot: entry.snapshot,
            });
        }
        let _ = fs::remove_file(path);
        self.invalidate_diagnostics();
        None
    }

    /// 写入本次解析结果和刷新时间；损坏缓存可在下次播放时自动重建。
    ///
    /// `generation` 用于排序：同一首歌可能有多轮解析在途（手动刷新、监听事件、切歌），
    /// 代数较小的写入属于已被取代的那一轮，直接丢弃，避免它覆盖同键的较新结果——
    /// 否则下次启动会读回旧内容，而内存里展示的是新内容。
    pub fn store(
        &self,
        snapshot: &LyricsSnapshot,
        chinese_variant: LyricsChineseVariant,
        generation: u64,
    ) -> Result<(), LyricsError> {
        self.store_at(snapshot, chinese_variant, generation, now_seconds())
    }

    /// 原位改写同一首歌的缓存字形，并保留歌词来源原本的刷新时间。
    pub fn rewrite_variant(
        &self,
        snapshot: &LyricsSnapshot,
        chinese_variant: LyricsChineseVariant,
        generation: u64,
        refreshed_at_seconds: u64,
    ) -> Result<(), LyricsError> {
        self.store_at(snapshot, chinese_variant, generation, refreshed_at_seconds)
    }

    fn store_at(
        &self,
        snapshot: &LyricsSnapshot,
        chinese_variant: LyricsChineseVariant,
        generation: u64,
        refreshed_at_seconds: u64,
    ) -> Result<(), LyricsError> {
        let Some(track_key) = snapshot.track_key.as_deref() else {
            return Ok(());
        };
        // 瞬时未命中不持久化；真实歌词、纯音乐与“没有歌词”都可跨播放复用。
        if !is_cacheable_status(snapshot.status) {
            return Ok(());
        }
        if !self.reserve_write(track_key, generation) {
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
            refreshed_at_seconds,
            chinese_variant,
            snapshot: snapshot.clone(),
        };
        let content = encode_entry(&cache_entry)?;
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
        match replace_file(&temporary, &target) {
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

    /// 记录本次写入的代数；已有更大代数的写入时返回 false，表示本轮已被取代。
    fn reserve_write(&self, track_key: &str, generation: u64) -> bool {
        let Ok(mut generations) = self.write_generations.lock() else {
            // 锁不可用时按允许写入处理：宁可偶发乱序，也不要静默丢掉歌词。
            return true;
        };
        if generations
            .get(track_key)
            .is_some_and(|written| *written > generation)
        {
            return false;
        }
        generations.insert(track_key.to_owned(), generation);
        true
    }

    /// 清空全部规范化歌词缓存，同时保留版本目录供后续写入复用。
    pub fn clear(&self) -> Result<(), std::io::Error> {
        self.ensure_directory_boundary()?;
        for entry in fs::read_dir(&self.cache_path)? {
            let entry = entry?;
            // 云同步占位符等特殊条目取不到元数据：跳过它，不能让单个条目让整次清理失败。
            let Ok(metadata) = filesystem::entry_metadata_without_reparse(&entry) else {
                continue;
            };
            if metadata.is_file() {
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
                let Ok(metadata) = filesystem::entry_metadata_without_reparse(&entry) else {
                    continue;
                };
                if !metadata.is_file() {
                    continue;
                }
                let path = entry.path();
                let should_remove = fs::read(&path)
                    .ok()
                    .and_then(|content| decode_entry(&content))
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
            .and_then(|content| decode_entry(&content))?;
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
        self.cache_path.join(format!("{track_key}.bin"))
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
            // 取不到元数据的条目（云同步占位符等）不计入容量，也不能让淘汰整体失败：
            // 否则 prune 报错会让已成功写入的 store 返回错误，把“写成功”报成“写失败”。
            let Ok(metadata) = filesystem::entry_metadata_without_reparse(&entry) else {
                continue;
            };
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
